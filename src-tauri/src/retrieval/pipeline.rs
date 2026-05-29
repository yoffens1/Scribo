//! Hybrid Retrieval Pipeline Module
//!
//! Orchestrates the full retrieval and fetching flow.

use crate::AppError;
use crate::DbState;
use crate::db::repos::fragments;
use crate::retrieval::types::{
    RetrievalConfig, RetrieveOptions, RetrieveFilters, SearchResult, FetchQuery, FetchResult,
    QueryVariant, VariantSource, RetrievalMode,
};
use crate::retrieval::search::{rrf, apply_term_boost};
use crate::retrieval::config_resolver::resolve_config;
use crate::retrieval::context::RetrievalContext;
use crate::retrieval::cache::cached_or_run;
use crate::retrieval::preprocess::maybe_translate_query;

/// Minimum BM25 hit count (for original query) below which we trigger translate-fallback.
/// If the first-pass FTS5 search returns fewer than this many results, we assume the
/// keyword stage has poor recall and attempt a translated query as a supplement.
const LOW_KEYWORD_RECALL_THRESHOLD: usize = 3;

/// Filters search results using the provided `RetrieveFilters` criteria (e.g., target note_id).
fn apply_filters(results: Vec<SearchResult>, filters: &Option<RetrieveFilters>) -> Vec<SearchResult> {
    if let Some(ref f) = filters {
        results.into_iter().filter(|r| {
            if let Some(ref note_id) = f.note_id {
                if r.fragment_ref.note_id != *note_id {
                    return false;
                }
            }
            true
        }).collect()
    } else {
        results
    }
}

/// The main hybrid retrieval function.
/// Receives a query string and returns a sorted list of hydrated `SearchResult`s.
pub async fn retrieve(
    state: &DbState,
    query: &str,
    query_embedding: Option<&[f32]>,
    config: &RetrievalConfig,
    options: &RetrieveOptions,
) -> Result<Vec<SearchResult>, AppError> {
    // Embedding model guard
    if let Ok(report) = crate::services::reindex::find_stale_notes(
        state,
        crate::ai::embedding::CURRENT_EMBEDDING_MODEL,
        crate::ai::embedding::CURRENT_DIM,
    ) {
        if !report.stale_notes.is_empty() {
            tracing::warn!(
                stale_count = report.stale_notes.len(),
                "Found notes indexed with an outdated model or dimension. Please run 'reindex' to update them."
            );
        }
    }

    let resolved_config = resolve_config(state, query, config);
    let ctx = RetrievalContext::new(state, query, &resolved_config, options);

    // 1. Preprocessing Stage (Translation, HyDE, Synonym Expansion)
    let variants = ctx.build_variants(query).await;

    let tuning = resolved_config.tuning.clone().unwrap_or_default();
    let top_k = options.top_k.unwrap_or(5);
    let over_fetch = (top_k * tuning.over_fetch_multiplier).min(tuning.over_fetch_cap);

    // 2. Retrieval Stage (Concurrent search loops + Batch Embedding calculations)
    let mut variant_lists = ctx
        .retrieve_per_variant(variants, query_embedding, over_fetch)
        .await?;

    // 2.5. Translate-Fallback: if keyword recall is poor for the original query,
    //      try a translated variant and merge its results into the fusion pool.
    //      Only triggers in Hybrid mode with an available LLM.
    if resolved_config.mode == RetrievalMode::Hybrid {
        let original_kw_hits: usize = variant_lists
            .first()
            .map(|(_, _, hits)| *hits)
            .unwrap_or(0);

        if original_kw_hits < LOW_KEYWORD_RECALL_THRESHOLD {
            if let Some(ref llm) = ctx.llm {
                let vault_lang = &ctx.vault_lang;
                let detected_lang = &ctx.detected_lang;

                // Only translate if the target language is different from the query language,
                // OR if detected lang is unknown (short query — whatlang unsure)
                let should_translate = detected_lang != vault_lang || detected_lang == "en" && variant_lists.iter().all(|(_, _, h)| *h < LOW_KEYWORD_RECALL_THRESHOLD);

                if should_translate {
                    let model_id = llm.config().model.clone();
                    let translated = cached_or_run(
                        state,
                        query,
                        &model_id,
                        "translation",
                        vault_lang,
                        maybe_translate_query(llm, query, vault_lang)
                    ).await;

                    if let Some(translated_text) = translated {
                        if !translated_text.trim().is_empty() && translated_text.trim().to_lowercase() != query.trim().to_lowercase() {
                            tracing::debug!(
                                original_kw_hits,
                                translated_query = %translated_text,
                                "Low keyword recall — adding translate-fallback variant"
                            );

                            let fallback_variant = vec![QueryVariant {
                                text: translated_text,
                                lang: vault_lang.clone(),
                                source: VariantSource::Translated,
                                weight: 0.8,
                                vector_only: false,
                            }];

                            if let Ok(mut fallback_lists) = ctx
                                .retrieve_per_variant(fallback_variant, query_embedding, over_fetch)
                                .await
                            {
                                variant_lists.append(&mut fallback_lists);
                            }
                        }
                    }
                }
            }
        }
    }

    // 3. Fusion Stage (Merge matching document ranks using Reciprocal Rank Fusion)
    let candidates_limit = if resolved_config.ai_rerank.as_ref().map(|r| r.enabled).unwrap_or(false) {
        top_k * tuning.rerank_pool_multiplier
    } else {
        top_k
    };
    // Strip the kw_hits field — rrf only needs (results, weight)
    let lists_for_rrf: Vec<(Vec<SearchResult>, f32)> = variant_lists
        .into_iter()
        .map(|(res, w, _)| (res, w))
        .collect();
    let mut fused = rrf(lists_for_rrf, tuning.rrf_k.unwrap_or(60.0), candidates_limit);

    // 3.5. Term Boost for Exact Matches
    let term_boost_weight = tuning.term_boost_weight.unwrap_or(crate::constants::DEFAULT_TERM_BOOST_WEIGHT);
    apply_term_boost(&mut fused, query, term_boost_weight);

    // 4. Filtering Stage
    fused = apply_filters(fused, &options.filters);

    // 5. Reranking Stage (LLM-driven listwise or scoring sort)
    ctx.rerank(&mut fused, query).await;

    // 5.5. Explain score breakdown (remove debug field if explain is false)
    let explain = options.explain.unwrap_or(false);
    if !explain {
        for r in &mut fused {
            r.debug = None;
        }
    }

    // 6. Filter by min_score_ratio (relative score cutoff)
    if let Some(ratio) = options.min_score_ratio {
        if let Some(best) = fused.first() {
            let threshold = best.score * ratio;
            fused.retain(|r| r.score >= threshold);
        }
    }

    fused.truncate(top_k);
    Ok(fused)
}

/// Fetches raw fragments and their structural data directly for admin or UI sync operations.
pub fn fetch(
    state: &DbState,
    query: &FetchQuery,
) -> Result<Vec<FetchResult>, AppError> {
    let include_deleted = query.include_deleted.unwrap_or(false);
    let raw = state.with_conn(|conn| {
        fragments::list_fragments_with_note(
            conn,
            query.note_id.map(|id| id.0),
            include_deleted,
            crate::constants::EMBEDDING_MODEL,
        )
    })?;

    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(raw.len());

    let page = if offset >= raw.len() {
        Vec::new()
    } else {
        let end = (offset + limit).min(raw.len());
        raw[offset..end].to_vec()
    };

    let results = page.into_iter().map(|ch| FetchResult {
        fragment_id: Some(ch.fragment.id.0),
        note_id: ch.fragment.note_id,
        fragment_index: ch.fragment.fragment_index,
        fragment_text: Some(ch.fragment.text_clean),
        token_count: None,
        embedding: ch.fragment.embedding.unwrap_or_default(),
    }).collect();

    Ok(results)
}

/// Helper to embed a query using the currently configured embedding model.
/// Handles query-specific formatting (e.g., instructions/prefixes) and L2-normalization.
pub async fn embed_query(_state: &DbState, text: &str) -> Result<Vec<f32>, AppError> {
    let embedder = crate::ai::embedding::Embedder::new(crate::ai::types::EmbedderConfig {
        provider: "local".to_string(),
        model: Some(crate::ai::embedding::CURRENT_EMBEDDING_MODEL.to_string()),
        api_key: None,
        base_url: None,
    });
    embedder.embed_query(text).await.map_err(AppError::Other)
}
