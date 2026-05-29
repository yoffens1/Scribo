//! # Per-Variant Search Executor
//!
//! Runs keyword (FTS5 BM25) and vector (cosine ANN) searches concurrently for each
//! [`QueryVariant`], then fuses the results with RRF into a single ranked list.

use std::collections::HashMap;
use std::sync::Arc;
use crate::AppError;
use crate::DbState;
use crate::db::repos::fragments;
use crate::retrieval::types::{
    QueryVariant, VariantSource, SearchResult, FragmentRef, RetrievalMode, RetrievalTuning,
};
use crate::retrieval::search::fusion::rrf;

/// Builds embeddings for all non-original query variants in a single batched LLM call.
pub async fn build_variant_embeddings(
    llm: Option<&Arc<crate::ai::LlmService>>,
    variants: &[QueryVariant],
) -> Result<HashMap<usize, Vec<f32>>, AppError> {
    let mut needs_embedding: Vec<(usize, String)> = Vec::new();
    let mut result: HashMap<usize, Vec<f32>> = HashMap::new();

    for (i, v) in variants.iter().enumerate() {
        if v.source != VariantSource::Original {
            needs_embedding.push((i, v.text.clone()));
        }
    }

    if needs_embedding.is_empty() {
        return Ok(result);
    }

    if let Some(llm) = llm {
        let texts: Vec<String> = needs_embedding.iter().map(|(_, t)| t.clone()).collect();
        match llm.generate_embeddings(texts).await {
            Ok(embs) => {
                for ((i, _), emb) in needs_embedding.iter().zip(embs) {
                    result.insert(*i, emb);
                }
            }
            Err(e) => {
                tracing::warn!("Failed to generate variant embeddings in batch: {:?}", e);
            }
        }
    }
    Ok(result)
}

/// Orchestrates retrieval across all query variants concurrently.
/// Embeds non-original variants in one batched call, then runs FTS5 and vector search
/// concurrently for each variant, fusing results per-variant with RRF.
pub async fn retrieve_per_variant(
    state: &DbState,
    llm: Option<&Arc<crate::ai::LlmService>>,
    variants: Vec<QueryVariant>,
    query_embedding: Option<&[f32]>,
    mode: RetrievalMode,
    embedding_weight: Option<f32>,
    tuning: RetrievalTuning,
    target_level: Option<i64>,
    over_fetch: usize,
) -> Result<Vec<(Vec<SearchResult>, f32)>, AppError> {
    let embeddings = build_variant_embeddings(llm, &variants).await?;
    let pool = state.pool.clone();

    let mut futures = Vec::new();
    for (i, v) in variants.into_iter().enumerate() {
        let emb = if v.source == VariantSource::Original {
            query_embedding.map(|e| e.to_vec())
        } else {
            embeddings.get(&i).cloned()
        };

        let pool = pool.clone();
        let tuning = tuning.clone();
        futures.push(async move {
            let results = retrieve_for_variant(
                pool,
                v.clone(),
                emb,
                mode,
                embedding_weight,
                tuning,
                target_level,
                over_fetch,
            ).await?;
            Ok::<(Vec<SearchResult>, f32), AppError>((results, v.weight))
        });
    }

    let variant_lists = futures::future::try_join_all(futures).await?;
    Ok(variant_lists)
}

/// Free function to handle the core keyword + vector database searches for a single QueryVariant.
/// Avoids any borrow gymnastics on RetrievalContext.
async fn retrieve_for_variant(
    pool: std::sync::Arc<parking_lot::RwLock<Option<r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>>>>,
    v: QueryVariant,
    emb: Option<Vec<f32>>,
    mode: RetrievalMode,
    embedding_weight: Option<f32>,
    tuning: RetrievalTuning,
    target_level: Option<i64>,
    over_fetch: usize,
) -> Result<Vec<SearchResult>, AppError> {
    // 1. Keyword search branch (FTS5 BM25)
    let keyword_future = {
        let text = v.text.clone();
        let vector_only = v.vector_only;
        let pool = pool.clone();
        tokio::task::spawn_blocking(move || {
            if !vector_only && (mode == RetrievalMode::Keyword || mode == RetrievalMode::Hybrid) {
                let pool_opt = pool.read();
                let pool_actual = pool_opt.as_ref().ok_or(AppError::NotInitialized)?;
                let mut conn = pool_actual.get().map_err(|e| AppError::Other(e.to_string()))?;
                match fragments::search(&mut conn, &text, over_fetch as i64) {
                    Ok(hits) => {
                        let results = hits.into_iter().map(|h| SearchResult {
                            fragment_ref: FragmentRef {
                                note_id: h.hit.note_id,
                                fragment_index: h.hit.fragment_index,
                            },
                            score: h.score,
                            text: Some(h.hit.text),
                            note_title: h.hit.note_title.clone(),
                            debug: None,
                        }).collect::<Vec<_>>();
                        Ok(results)
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, query = %text, "Keyword search failed");
                        Err(e)
                    }
                }
            } else {
                Ok(Vec::new())
            }
        })
    };

    // 2. Vector search branch (Cosine similarity scanning)
    let vector_future = {
        let emb_bytes = emb.map(|e| bytemuck::cast_slice::<f32, u8>(&e).to_vec());
        let pool = pool.clone();
        tokio::task::spawn_blocking(move || {
            if mode == RetrievalMode::Embedding || mode == RetrievalMode::Hybrid {
                if let Some(bytes) = emb_bytes {
                    // TODO: Replace brute-force scan with sqlite-vec extension for O(log n) ANN.
                    let start = std::time::Instant::now();

                    let pool_opt = pool.read();
                    let pool_actual = pool_opt.as_ref().ok_or(AppError::NotInitialized)?;
                    let mut conn = pool_actual.get().map_err(|e| AppError::Other(e.to_string()))?;
                    let res = match fragments::vector_search(&mut conn, &bytes, target_level, over_fetch, crate::constants::EMBEDDING_MODEL, crate::constants::INDEXING_VERSION) {
                        Ok(hits) => {
                            let results = hits.into_iter().map(|h| SearchResult {
                                fragment_ref: FragmentRef {
                                    note_id: h.hit.note_id,
                                    fragment_index: h.hit.fragment_index,
                                },
                                score: h.score,
                                text: Some(h.hit.text),
                                note_title: h.hit.note_title.clone(),
                                debug: None,
                            }).collect::<Vec<_>>();
                            Ok(results)
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "Vector search failed");
                            Err(e)
                        }
                    };

                    tracing::debug!(
                        duration_ms = start.elapsed().as_millis(),
                        "Vector scan execution time"
                    );
                    res
                } else {
                    Ok(Vec::new())
                }
            } else {
                Ok(Vec::new())
            }
        })
    };

    // Join both branches concurrently
    let (kw_res, vec_res) = futures::future::join(keyword_future, vector_future).await;

    let keyword_results = kw_res.map_err(|e| AppError::Other(e.to_string()))??;
    let vector_results = vec_res.map_err(|e| AppError::Other(e.to_string()))??;

    // Perform Reciprocal Rank Fusion (RRF) on hybrid variants
    let results = if mode == RetrievalMode::Hybrid {
        rrf(
            vec![
                (keyword_results, 1.0),
                (vector_results, embedding_weight.unwrap_or(1.0)),
            ],
            tuning.rrf_k.unwrap_or(crate::constants::DEFAULT_RRF_K),
            over_fetch,
        )
    } else if mode == RetrievalMode::Keyword {
        keyword_results
    } else {
        vector_results
    };

    Ok(results)
}
