use std::collections::HashMap;
use std::sync::Arc;
use crate::AppError;
use crate::DbState;
use crate::ai::LlmService;
use crate::db::repos::fragments;
use crate::lang::{detect_language, pick_dominant_language};
use crate::retrieval::types::{
    RetrievalConfig, RetrieveOptions, SearchResult, FragmentRef,
    RetrievalMode, SynonymExpansion, RerankMode, VariantSource, QueryVariant,
    RetrievalTuning,
};
use crate::retrieval::fusion::rrf;
use crate::retrieval::stages::{
    hyde::run_hyde,
    synonyms::{expand_static, expand_llm},
    translate_query::maybe_translate_query,
};
use crate::retrieval::rerankers::{
    listwise::rerank_listwise,
    scoring::rerank_scoring,
};
use crate::retrieval::cache::cached_or_run;

use std::sync::OnceLock;

static SYNONYMS: OnceLock<HashMap<String, Vec<String>>> = OnceLock::new();

pub fn default_synonyms() -> &'static HashMap<String, Vec<String>> {
    SYNONYMS.get_or_init(|| {
        let json_str = include_str!("../../data/synonyms.json");
        serde_json::from_str(json_str).unwrap_or_default()
    })
}

/// Detects and returns the dominant language of the vault.
/// Scans the first sample chunks, parses their language, and caches the result in the `DbState`.
pub fn get_vault_language(state: &DbState) -> String {
    if let Some(cached) = state.cached_vault_lang.read().as_ref() {
        return cached.clone();
    }

    let fragments = state.with_conn(|conn| {
        fragments::get_sample_texts(conn, crate::constants::VAULT_LANG_SAMPLE_SIZE as i64)
    }).unwrap_or_default();

    let best_lang = pick_dominant_language(&fragments);

    *state.cached_vault_lang.write() = Some(best_lang.clone());
    best_lang
}

/// Deduplicates query variants and fuses their weights.
pub fn dedup_variants(variants: Vec<QueryVariant>) -> Vec<QueryVariant> {
    let mut seen: HashMap<(String, String), QueryVariant> = HashMap::new();
    for v in variants {
        let key = (v.text.trim().to_lowercase(), v.lang.clone());
        match seen.entry(key) {
            std::collections::hash_map::Entry::Vacant(e) => {
                e.insert(v);
            }
            std::collections::hash_map::Entry::Occupied(mut e) => {
                let existing = e.get_mut();
                existing.weight = existing.weight.max(v.weight) + 0.1 * existing.weight.min(v.weight);
            }
        }
    }
    seen.into_values().collect()
}

/// The execution context for a single retrieval query request.
pub struct RetrievalContext<'a> {
    pub state: &'a DbState,
    pub llm: Option<Arc<LlmService>>,
    pub config: &'a RetrievalConfig,
    pub options: &'a RetrieveOptions,
    pub detected_lang: String,
    pub vault_lang: String,
    pub tuning: RetrievalTuning,
}

impl<'a> RetrievalContext<'a> {
    /// Creates a new retrieval execution context, resolving target languages and locating/initializing
    /// the cached LLM service from application state.
    pub fn new(
        state: &'a DbState,
        query: &str,
        config: &'a RetrievalConfig,
        options: &'a RetrieveOptions,
    ) -> Self {
        let detected_lang = detect_language(query).unwrap_or_else(|| "en".to_string());
        let vault_lang = config.vault_lang.clone().unwrap_or_else(|| get_vault_language(state));

        // Use cached LlmService if config matches; otherwise initialize a new one and cache it.
        let llm = if let Some(llm_cfg) = &config.llm_config {
            Some(state.get_llm_service(llm_cfg, None))
        } else {
            let guard = state.llm_service.read();
            guard.as_ref().map(|(_, s)| s.clone())
        };

        let tuning = config.tuning.clone().unwrap_or_default();

        Self {
            state,
            llm,
            config,
            options,
            detected_lang,
            vault_lang,
            tuning,
        }
    }

    /// Translates the query if the detected language differs from the vault language.
    async fn variant_translation(&self, query: &str) -> Option<QueryVariant> {
        let pipeline = self.config.pipeline.as_ref();
        let auto_translate = pipeline.and_then(|p| p.auto_translate).unwrap_or(false);
        if auto_translate && self.detected_lang != self.vault_lang {
            if let Some(ref llm) = self.llm {
                let model_id = llm.config().model.clone();
                let translated = cached_or_run(
                    self.state,
                    query,
                    &model_id,
                    "translation",
                    &self.vault_lang,
                    maybe_translate_query(llm, query, &self.vault_lang)
                ).await;

                if let Some(text) = translated {
                    return Some(QueryVariant {
                        text,
                        lang: self.vault_lang.clone(),
                        source: VariantSource::Translated,
                        weight: 1.0,
                        vector_only: false,
                    });
                }
            }
        }
        None
    }

    /// Generates hypothetical document embeddings variant using LLM.
    async fn variant_hyde(&self, query: &str) -> Option<QueryVariant> {
        let pipeline = self.config.pipeline.as_ref();
        let hyde_enabled = pipeline.and_then(|p| p.hyde).unwrap_or(false);
        if hyde_enabled {
            if let Some(ref llm) = self.llm {
                let model_id = llm.config().model.clone();
                let auto_translate = pipeline.and_then(|p| p.auto_translate).unwrap_or(false);
                let target_lang = if auto_translate { &self.vault_lang } else { &self.detected_lang };

                let hyde_text = cached_or_run(
                    self.state,
                    query,
                    &model_id,
                    "hyde",
                    target_lang,
                    run_hyde(llm, query, target_lang)
                ).await;

                if let Some(text) = hyde_text {
                    return Some(QueryVariant {
                        text,
                        lang: target_lang.clone(),
                        source: VariantSource::Hyde,
                        weight: self.tuning.hyde_weight,
                        vector_only: true,
                    });
                }
            }
        }
        None
    }

    /// Expands variants with static dictionary-based or LLM-driven synonyms.
    async fn variants_synonyms(&self, base: &[QueryVariant], query: &str) -> Vec<QueryVariant> {
        let mut new_syns = Vec::new();
        let pipeline = self.config.pipeline.as_ref();
        let expand_synonyms = pipeline.and_then(|p| p.expand_synonyms).unwrap_or(SynonymExpansion::Off);

        if expand_synonyms == SynonymExpansion::Static {
            let default_dict = default_synonyms();
            let dict = pipeline.and_then(|p| p.synonym_dict.as_ref()).unwrap_or(default_dict);
            for v in base {
                if v.vector_only {
                    continue;
                }
                let syns = expand_static(&v.text, dict);
                for syn in syns {
                    new_syns.push(QueryVariant {
                        text: syn,
                        lang: v.lang.clone(),
                        source: VariantSource::Synonym,
                        weight: self.tuning.synonym_weight,
                        vector_only: false,
                    });
                }
            }
        } else if expand_synonyms == SynonymExpansion::Llm {
            if let Some(ref llm) = self.llm {
                let auto_translate = pipeline.and_then(|p| p.auto_translate).unwrap_or(false);
                let target_lang = if auto_translate { &self.vault_lang } else { &self.detected_lang };
                let syns = expand_llm(llm, query, target_lang).await;
                for syn in syns {
                    new_syns.push(QueryVariant {
                        text: syn,
                        lang: target_lang.clone(),
                        source: VariantSource::Synonym,
                        weight: self.tuning.synonym_weight,
                        vector_only: false,
                    });
                }
            }
        }
        new_syns
    }

    /// Preprocesses the user's query into multiple QueryVariants.
    pub async fn build_variants(&self, query: &str) -> Vec<QueryVariant> {
        let mut variants = vec![QueryVariant {
            text: query.to_string(),
            lang: self.detected_lang.clone(),
            source: VariantSource::Original,
            weight: 1.0,
            vector_only: false,
        }];

        // 1. Translation Stage
        if let Some(translated) = self.variant_translation(query).await {
            variants.push(translated);
        }

        // 2. HyDE Stage
        if let Some(hyde_variant) = self.variant_hyde(query).await {
            variants.push(hyde_variant);
        }

        // 3. Synonym Expansion Stage
        let synonyms = self.variants_synonyms(&variants, query).await;
        variants.extend(synonyms);

        // Deduplicate query variants and combine matching query weights
        let deduped = dedup_variants(variants);

        // Apply language-specific adaptive scaling weights
        let mut weighted = Vec::new();
        let adaptive = self.config.adaptive_weights.clone().unwrap_or_default();
        for mut v in deduped {
            if v.lang == self.vault_lang {
                v.weight *= adaptive.same_lang;
            } else {
                v.weight *= adaptive.other_lang;
            }
            weighted.push(v);
        }

        weighted
    }

    async fn build_variant_embeddings(
        &self,
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

        if let Some(ref llm) = self.llm {
            let texts: Vec<String> = needs_embedding.iter().map(|(_, t)| t.clone()).collect();
            match llm.generate_embeddings(texts).await {
                Ok(embs) => {
                    for ((i, _), emb) in needs_embedding.iter().zip(embs) {
                        result.insert(*i, emb);
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to generate embeddings in batch: {:?}", e);
                }
            }
        }
        Ok(result)
    }

    /// Orchestrates retrieval across all query variants concurrently.
    pub async fn retrieve_per_variant(
        &self,
        variants: Vec<QueryVariant>,
        query_embedding: Option<&[f32]>,
        over_fetch: usize,
    ) -> Result<Vec<(Vec<SearchResult>, f32)>, AppError> {
        let embeddings = self.build_variant_embeddings(&variants).await?;
        let mode = self.config.mode;
        let embedding_weight = self.config.embedding_weight;
        let pool = self.state.pool.clone();
        let target_level = self.options.target_level;
        let tuning = self.tuning.clone();

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

    /// Performs LLM-driven reranking on top candidates (Scoring or Listwise).
    pub async fn rerank(&self, fused: &mut Vec<SearchResult>, query: &str) {
        let rerank_config = self.config.ai_rerank.as_ref();
        if let Some(rc) = rerank_config {
            if rc.enabled && !fused.is_empty() {
                if let Some(ref llm) = self.llm {
                    let max_cands = rc.max_candidates.unwrap_or(25).min(fused.len());
                    let rerank_mode = rc.mode.unwrap_or(RerankMode::Scoring);

                    if rerank_mode == RerankMode::Listwise {
                        match rerank_listwise(llm, query, &fused[..max_cands]).await {
                            Some(reranked) => {
                                let mut final_fused = reranked;
                                if fused.len() > max_cands {
                                    final_fused.extend(fused[max_cands..].iter().cloned());
                                }
                                *fused = final_fused;
                            }
                            None => {
                                tracing::warn!("Listwise reranking failed or returned None");
                            }
                        }
                    } else {
                        // Scoring mode
                        let (to_sort, _rest) = fused.split_at_mut(max_cands);
                        rerank_scoring(llm, query, to_sort, self.tuning.scoring_max_score).await;
                    }
                }
            }
        }
    }
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
        let target_level = target_level;
        let pool = pool.clone();
        tokio::task::spawn_blocking(move || {
            if mode == RetrievalMode::Embedding || mode == RetrievalMode::Hybrid {
                if let Some(bytes) = emb_bytes {
                    // TODO: Optimize brute-force cosine scanning.
                    // Currently, this performs a linear scan over all fragment vectors.
                    // In the future, we should:
                    // 1. Integrate sqlite-vec extension for indexed vector search.
                    // 2. Use a BinaryHeap for early-exit scanning.
                    let start = std::time::Instant::now();
                    
                    let pool_opt = pool.read();
                    let pool_actual = pool_opt.as_ref().ok_or(AppError::NotInitialized)?;
                    let mut conn = pool_actual.get().map_err(|e| AppError::Other(e.to_string()))?;
                    let res = match fragments::vector_search(&mut conn, &bytes, target_level, over_fetch) {
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

    // Join both searches concurrently
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval::types::{QueryVariant, VariantSource};

    #[test]
    fn test_dedup_variants_weights() {
        let variants = vec![
            QueryVariant {
                text: "Query text".to_string(),
                lang: "en".to_string(),
                source: VariantSource::Original,
                weight: 1.0,
                vector_only: false,
            },
            QueryVariant {
                text: "query text ".to_string(), // trailing spaces, mixed case
                lang: "en".to_string(),
                source: VariantSource::Synonym,
                weight: 0.6,
                vector_only: false,
            },
            QueryVariant {
                text: "Another".to_string(),
                lang: "en".to_string(),
                source: VariantSource::Translated,
                weight: 0.8,
                vector_only: false,
            },
        ];

        let deduped = dedup_variants(variants);
        assert_eq!(deduped.len(), 2);

        // Find the "query text" variant
        let qt = deduped.iter().find(|v| v.text.trim().to_lowercase() == "query text").unwrap();
        // weight calculation: max(1.0, 0.6) + 0.1 * min(1.0, 0.6) = 1.0 + 0.06 = 1.06
        assert!((qt.weight - 1.06).abs() < 1e-5);

        let another = deduped.iter().find(|v| v.text == "Another").unwrap();
        assert_eq!(another.weight, 0.8);
    }
}
