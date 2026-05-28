//! Hybrid Retrieval Pipeline Module
//!
//! This module implements the main retrieval engine of Scribo. It supports:
//! 1. **Multi-Stage Query Preprocessing**: Auto-translation, HyDE (Hypothetical Document Embeddings),
//!    and static or LLM-driven synonym expansion.
//! 2. **Batch Embedding Generation**: Minimizes network latency by batching all generated query
//!    variants into a single LLM API request.
//! 3. **Non-Blocking SQLite Concurrency**: Uses `tokio::task::spawn_blocking` to execute database queries,
//!    ensuring the tokio runtime thread pool is never blocked.
//! 4. **Parallel Pipeline Execution**: Runs vector and keyword searches concurrently for each query variant
//!    (`futures::future::join`) and scales parallel execution across all variants using `try_join_all`.
//! 5. **Hybrid Fusion & Reranking**: Merges keyword and vector scores using Reciprocal Rank Fusion (RRF)
//!    and applies LLM-driven reranking (Scoring or Listwise mode) to select the final top results.
//! 6. **Zero-Allocation Text Hydration**: Fetches the clean text directly during the FTS/Vector search JOINs,
//!    avoiding separate N+1 hydration queries.

use std::collections::HashMap;
use std::sync::Arc;
use crate::AppError;
use crate::DbState;
use crate::db::repos::fragments;
use crate::ai::LlmService;
use crate::retrieval::language::detect_language;
use crate::retrieval::types::{
    RetrievalConfig, RetrieveOptions, RetrieveFilters, SearchResult, FragmentRef, FetchQuery,
    FetchResult, RetrievalMode, SynonymExpansion, RerankMode, VariantSource, QueryVariant,
};
use crate::retrieval::fusion::rrf;
use crate::retrieval::stages::{
    hyde::run_hyde,
    synonyms::{expand_static, expand_llm},
    translate_query::run_translation,
};
use crate::retrieval::rerankers::{
    listwise::rerank_listwise,
    scoring::rerank_scoring,
};

/// Returns the default static synonym dictionary.
/// Lazily loads the mappings from the embedded JSON configuration file on the first call.
pub fn default_synonyms() -> &'static HashMap<String, Vec<String>> {
    static SYNONYMS: std::sync::OnceLock<HashMap<String, Vec<String>>> = std::sync::OnceLock::new();
    SYNONYMS.get_or_init(|| {
        let json_str = include_str!("../../data/synonyms.json");
        serde_json::from_str(json_str).unwrap_or_default()
    })
}

/// Detects and returns the dominant language of the vault.
/// Scans the first 50 chunks, parses their language, and caches the result in the `DbState`.
fn get_vault_language(state: &DbState) -> String {
    // Return cached language if already computed to avoid redundant DB queries
    if let Some(cached) = state.cached_vault_lang.read().as_ref() {
        return cached.clone();
    }

    let fragments = state.with_conn(|conn| {
        let mut stmt = conn.prepare("SELECT clean_text FROM chunks WHERE level = 1 AND clean_text IS NOT NULL LIMIT 50")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut res = Vec::new();
        for r in rows {
            res.push(r?);
        }
        Ok(res)
    }).unwrap_or_default();

    if fragments.is_empty() {
        return "en".to_string();
    }

    let mut counts = HashMap::new();
    for text in fragments {
        if text.trim().len() < 10 {
            continue;
        }
        if let Some(lang) = detect_language(&text) {
            *counts.entry(lang).or_insert(0) += 1;
        }
    }

    let mut best_lang = "en".to_string();
    let mut max_count = 0;
    for (l, c) in counts {
        if c > max_count {
            max_count = c;
            best_lang = l;
        }
    }

    // Cache the detected language for subsequent requests
    *state.cached_vault_lang.write() = Some(best_lang.clone());

    best_lang
}

/// Deduplicates query variants and fuses their weights.
/// If a variant text matches an existing one, their combined weight is calculated as:
/// `weight = max(w1, w2) + 0.1 * min(w1, w2)`
fn dedup_variants(variants: Vec<QueryVariant>) -> Vec<QueryVariant> {
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

/// The execution context for a single retrieval query request.
struct RetrievalContext<'a> {
    state: &'a DbState,
    llm: Option<Arc<LlmService>>,
    config: &'a RetrievalConfig,
    options: &'a RetrieveOptions,
    detected_lang: String,
    vault_lang: String,
}

impl<'a> RetrievalContext<'a> {
    /// Creates a new retrieval execution context, resolving target languages and locating/initializing
    /// the cached LLM service from application state.
    fn new(
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

        Self {
            state,
            llm,
            config,
            options,
            detected_lang,
            vault_lang,
        }
    }

    /// Preprocesses the user's query into multiple QueryVariants by executing:
    /// 1. Translation: Translates the query if the detected language differs from the vault language.
    /// 2. HyDE: Generates a hypothetical response/context using the LLM.
    /// 3. Synonym Expansion: Performs dictionary-based or LLM-driven term expansion.
    async fn build_variants(&self, query: &str) -> Vec<QueryVariant> {
        let mut variants = vec![QueryVariant {
            text: query.to_string(),
            lang: self.detected_lang.clone(),
            source: VariantSource::Original,
            weight: 1.0,
            vector_only: false,
        }];

        let pipeline = self.config.pipeline.as_ref();

        // 1. Translation Stage
        let auto_translate = pipeline.and_then(|p| p.auto_translate).unwrap_or(false);
        if auto_translate && self.detected_lang != self.vault_lang {
            if let Some(ref llm) = self.llm {
                match run_translation(llm, query, &self.vault_lang).await {
                    Some(translated) => {
                        variants.push(QueryVariant {
                            text: translated,
                            lang: self.vault_lang.clone(),
                            source: VariantSource::Translated,
                            weight: 1.0,
                            vector_only: false,
                        });
                    }
                    None => {
                        tracing::warn!("Translation failed for query: {}", query);
                    }
                }
            }
        }

        // 2. HyDE Stage (Hypothetical Document Embeddings)
        let hyde_enabled = pipeline.and_then(|p| p.hyde).unwrap_or(false);
        if hyde_enabled {
            if let Some(ref llm) = self.llm {
                let target_lang = if auto_translate { &self.vault_lang } else { &self.detected_lang };
                match run_hyde(llm, query, target_lang).await {
                    Some(hyde_text) => {
                        let tuning = self.config.tuning.clone().unwrap_or_default();
                        variants.push(QueryVariant {
                            text: hyde_text,
                            lang: target_lang.clone(),
                            source: VariantSource::Hyde,
                            weight: tuning.hyde_weight,
                            vector_only: true, // Marked as vector_only since synthetic response isn't suitable for FTS5 keyword index matching.
                        });
                    }
                    None => {
                        tracing::warn!("HyDE generation failed for query: {}", query);
                    }
                }
            }
        }

        // 3. Synonym Expansion Stage
        let expand_synonyms = pipeline.and_then(|p| p.expand_synonyms).unwrap_or(SynonymExpansion::Off);
        let tuning = self.config.tuning.clone().unwrap_or_default();
        if expand_synonyms == SynonymExpansion::Static {
            let default_dict = default_synonyms();
            let dict = pipeline.and_then(|p| p.synonym_dict.as_ref()).unwrap_or(default_dict);
            let mut new_syns = Vec::new();
            for v in &variants {
                if v.vector_only {
                    continue; // Skip synonym expansion on synthetic/vector-only queries (HyDE)
                }
                let syns = expand_static(&v.text, dict);
                for syn in syns {
                    new_syns.push(QueryVariant {
                        text: syn,
                        lang: v.lang.clone(),
                        source: VariantSource::Synonym,
                        weight: tuning.synonym_weight,
                        vector_only: false,
                    });
                }
            }
            variants.extend(new_syns);
        } else if expand_synonyms == SynonymExpansion::Llm {
            if let Some(ref llm) = self.llm {
                let target_lang = if auto_translate { &self.vault_lang } else { &self.detected_lang };
                let syns = expand_llm(llm, query, target_lang).await;
                for syn in syns {
                    variants.push(QueryVariant {
                        text: syn,
                        lang: target_lang.clone(),
                        source: VariantSource::Synonym,
                        weight: tuning.synonym_weight,
                        vector_only: false,
                    });
                }
            }
        }

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

    /// Batch embeds all query variants in a single API call to reduce latency.
    async fn build_variant_embeddings(
        &self,
        variants: &[QueryVariant],
        query_embedding: Option<&[f32]>,
    ) -> HashMap<usize, Vec<f32>> {
        let mut needs_embedding: Vec<(usize, String)> = Vec::new();
        let mut result: HashMap<usize, Vec<f32>> = HashMap::new();

        for (i, v) in variants.iter().enumerate() {
            // Reuse the original query embedding if provided by the frontend
            if v.source == VariantSource::Original {
                if let Some(e) = query_embedding {
                    result.insert(i, e.to_vec());
                    continue;
                }
            }
            needs_embedding.push((i, v.text.clone()));
        }

        if needs_embedding.is_empty() {
            return result;
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
        result
    }

    /// Performs parallel database search (keyword and vector) for a single query variant.
    /// Uses `tokio::task::spawn_blocking` to decouple SQLite interactions from the async executor.
    async fn retrieve_for_variant(
        &self,
        v: &QueryVariant,
        emb: Option<Vec<f32>>,
        over_fetch: usize,
    ) -> Result<Vec<SearchResult>, AppError> {
        let mode = self.config.mode;
        let pool = self.state.pool.clone();

        // 1. Keyword search branch (FTS5)
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
                                text: Some(h.hit.text), // Text is fetched directly, hydration not needed!
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
            let target_level = self.options.target_level;
            let pool = pool.clone();
            tokio::task::spawn_blocking(move || {
                if mode == RetrievalMode::Embedding || mode == RetrievalMode::Hybrid {
                    if let Some(bytes) = emb_bytes {
                        let pool_opt = pool.read();
                        let pool_actual = pool_opt.as_ref().ok_or(AppError::NotInitialized)?;
                        let mut conn = pool_actual.get().map_err(|e| AppError::Other(e.to_string()))?;
                        match fragments::vector_search(&mut conn, &bytes, target_level, over_fetch) {
                            Ok(hits) => {
                                let results = hits.into_iter().map(|h| SearchResult {
                                    fragment_ref: FragmentRef {
                                        note_id: h.hit.note_id,
                                        fragment_index: h.hit.fragment_index,
                                    },
                                    score: h.score,
                                    text: Some(h.hit.text), // Text fetched directly in SQL JOIN.
                                }).collect::<Vec<_>>();
                                Ok(results)
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, "Vector search failed");
                                Err(e)
                            }
                        }
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
            let tuning = self.config.tuning.clone().unwrap_or_default();
            rrf(
                vec![
                    (keyword_results, 1.0),
                    (vector_results, self.config.embedding_weight.unwrap_or(1.0)),
                ],
                tuning.rrf_k,
                over_fetch,
            )
        } else if mode == RetrievalMode::Keyword {
            keyword_results
        } else {
            vector_results
        };

        Ok(results)
    }

    /// Orchestrates retrieval across all query variants concurrently.
    async fn retrieve_per_variant(
        &self,
        variants: Vec<QueryVariant>,
        query_embedding: Option<&[f32]>,
        over_fetch: usize,
    ) -> Result<Vec<(Vec<SearchResult>, f32)>, AppError> {
        let embeddings = self.build_variant_embeddings(&variants, query_embedding).await;

        let mut futures = Vec::new();
        for (i, v) in variants.iter().enumerate() {
            let emb = embeddings.get(&i).cloned();
            let self_clone = self;
            futures.push(async move {
                let results = self_clone.retrieve_for_variant(v, emb, over_fetch).await?;
                Ok::<(Vec<SearchResult>, f32), AppError>((results, v.weight))
            });
        }

        // Execute all variant searches in parallel
        let variant_lists = futures::future::try_join_all(futures).await?;

        Ok(variant_lists)
    }

    /// Performs LLM-driven reranking on top candidates (Scoring or Listwise).
    async fn rerank(&self, fused: &mut Vec<SearchResult>, query: &str) {
        let rerank_config = self.config.ai_rerank.as_ref();
        if let Some(rc) = rerank_config {
            if rc.enabled && !fused.is_empty() {
                if let Some(ref llm) = self.llm {
                    let tuning = self.config.tuning.clone().unwrap_or_default();
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
                        rerank_scoring(llm, query, to_sort, tuning.scoring_max_score).await;
                    }
                }
            }
        }
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
    let ctx = RetrievalContext::new(state, query, config, options);

    // 1. Preprocessing Stage (Translation, HyDE, Synonym Expansion)
    let variants = ctx.build_variants(query).await;

    let tuning = config.tuning.clone().unwrap_or_default();
    let top_k = options.top_k.unwrap_or(5);
    let over_fetch = (top_k * tuning.over_fetch_multiplier).min(tuning.over_fetch_cap);

    // 2. Retrieval Stage (Concurrent search loops + Batch Embedding calculations)
    let variant_lists = ctx
        .retrieve_per_variant(variants, query_embedding, over_fetch)
        .await?;

    // 3. Fusion Stage (Merge matching document ranks using Reciprocal Rank Fusion)
    let candidates_limit = if config.ai_rerank.as_ref().map(|r| r.enabled).unwrap_or(false) {
        top_k * tuning.rerank_pool_multiplier
    } else {
        top_k
    };
    let mut fused = rrf(variant_lists, tuning.rrf_k, candidates_limit);

    // 4. Filtering Stage
    fused = apply_filters(fused, &options.filters);

    // 5. Reranking Stage (LLM-driven listwise or scoring sort)
    ctx.rerank(&mut fused, query).await;

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
        token_count: ch.fragment.token_count,
        embedding: ch.fragment.embedding.unwrap_or_default(),
    }).collect();

    Ok(results)
}
