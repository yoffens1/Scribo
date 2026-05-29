//! # Retrieval Context
//!
//! A thin coordinator that holds the resolved state for a single retrieval request.
//! Heavy logic lives in [`preprocess`] and [`search`]; this struct only wires them together.

use std::sync::Arc;
use crate::AppError;
use crate::DbState;
use crate::ai::LlmService;
use crate::lang::detect_language;
use crate::retrieval::types::{
    RetrievalConfig, RetrieveOptions, SearchResult,
    RerankMode, RetrievalTuning, QueryVariant,
};
use crate::retrieval::config_resolver::resolve_config;
use crate::retrieval::preprocess;
use crate::retrieval::search;
use crate::retrieval::rerank::{rerank_listwise, rerank_scoring};

/// Thin coordinator for a single retrieval request.
/// Holds resolved config, language context, and LLM handle.
pub struct RetrievalContext<'a> {
    pub state: &'a DbState,
    pub llm: Option<Arc<LlmService>>,
    pub config: RetrievalConfig,
    pub options: &'a RetrieveOptions,
    pub detected_lang: String,
    pub vault_lang: String,
    pub tuning: RetrievalTuning,
}

impl<'a> RetrievalContext<'a> {
    /// Creates and resolves the retrieval context, loading calibrated settings from DB and
    /// auto-detecting the vault language if not set in the config.
    pub fn new(
        state: &'a DbState,
        query: &str,
        config: &'a RetrievalConfig,
        options: &'a RetrieveOptions,
    ) -> Self {
        let resolved_config = resolve_config(state, query, config);
        let detected_lang = detect_language(query).unwrap_or_else(|| "en".to_string());
        let vault_lang = resolved_config.vault_lang.clone()
            .unwrap_or_else(|| search::get_vault_language(state));

        let llm = if let Some(llm_cfg) = &resolved_config.llm_config {
            Some(state.get_llm_service(llm_cfg, None))
        } else {
            let guard = state.llm_service.read();
            guard.as_ref().map(|(_, s)| s.clone())
        };

        let tuning = resolved_config.tuning.clone().unwrap_or_default();

        Self {
            state,
            llm,
            config: resolved_config,
            options,
            detected_lang,
            vault_lang,
            tuning,
        }
    }

    /// Delegates to [`preprocess::build_variants`] with the context's resolved state.
    pub async fn build_variants(&self, query: &str) -> Vec<QueryVariant> {
        preprocess::build_variants(
            self.state,
            self.llm.as_ref(),
            &self.config,
            &self.tuning,
            query,
            &self.detected_lang,
            &self.vault_lang,
        ).await
    }

    /// Delegates to [`search::retrieve_per_variant`] with the context's resolved state.
    pub async fn retrieve_per_variant(
        &self,
        variants: Vec<QueryVariant>,
        query_embedding: Option<&[f32]>,
        over_fetch: usize,
    ) -> Result<Vec<(Vec<SearchResult>, f32, usize)>, AppError> {
        search::retrieve_per_variant(
            self.state,
            self.llm.as_ref(),
            variants,
            query_embedding,
            self.config.mode,
            self.config.embedding_weight,
            self.tuning.clone(),
            self.options.target_level,
            over_fetch,
        ).await
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
                        let (to_sort, _rest) = fused.split_at_mut(max_cands);
                        rerank_scoring(llm, query, to_sort, self.tuning.scoring_max_score).await;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::retrieval::preprocess::dedup_variants;
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
                text: "query text ".to_string(),
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

        let qt = deduped.iter().find(|v| v.text.trim().to_lowercase() == "query text").unwrap();
        assert!((qt.weight - 1.06).abs() < 1e-5);

        let another = deduped.iter().find(|v| v.text == "Another").unwrap();
        assert_eq!(another.weight, 0.8);
    }
}
