use crate::DbState;
use crate::retrieval::types::{RetrievalConfig, RetrievalTuning, PipelineConfig, SynonymExpansion};
use crate::db::repos::meta;
use crate::constants::*;

/// Resolves default configurations, database calibrated settings, and LLM providers.
pub fn resolve_config(state: &DbState, query: &str, config: &RetrievalConfig) -> RetrievalConfig {
    let mut resolved_config = config.clone();

    // 0. Load calibrated settings from DB meta table if not explicitly provided in config
    let (db_emb_weight, db_rrf_k, db_term_boost) = state.with_conn(|conn| {
        let emb_w = meta::get_f32(conn, "retrieval_embedding_weight")?;
        let rrf_k = meta::get_f32(conn, "retrieval_rrf_k")?;
        let term_boost = meta::get_f32(conn, "retrieval_term_boost_weight")?;
        Ok((emb_w, rrf_k, term_boost))
    }).unwrap_or((None, None, None));

    if resolved_config.embedding_weight.is_none() {
        resolved_config.embedding_weight = db_emb_weight;
    }
    if resolved_config.tuning.is_none() {
        let mut tuning = RetrievalTuning::default();
        tuning.rrf_k = Some(db_rrf_k.unwrap_or(DEFAULT_RRF_K));
        tuning.term_boost_weight = Some(db_term_boost.unwrap_or(DEFAULT_TERM_BOOST_WEIGHT));
        resolved_config.tuning = Some(tuning);
    } else if let Some(ref mut tuning) = resolved_config.tuning {
        if tuning.rrf_k.is_none() {
            tuning.rrf_k = Some(db_rrf_k.unwrap_or(DEFAULT_RRF_K));
        }
        if tuning.term_boost_weight.is_none() {
            tuning.term_boost_weight = Some(db_term_boost.unwrap_or(DEFAULT_TERM_BOOST_WEIGHT));
        }
    }

    // 0.5. Resolve default pipeline and LLM config if preprocessing/reranking is needed
    let word_count = query.split_whitespace().count();
    if resolved_config.pipeline.is_none() {
        resolved_config.pipeline = Some(PipelineConfig {
            auto_translate: Some(true),
            expand_synonyms: Some(SynonymExpansion::Off),
            synonym_dict: None,
            hyde: Some(word_count < HYDE_WORD_THRESHOLD),
        });
    } else if let Some(ref mut p) = resolved_config.pipeline {
        if p.auto_translate.is_none() {
            p.auto_translate = Some(true);
        }
        if p.hyde.is_none() {
            p.hyde = Some(word_count < HYDE_WORD_THRESHOLD);
        }
    }

    let needs_llm = resolved_config.ai_rerank.as_ref().map(|r| r.enabled).unwrap_or(false)
        || resolved_config.pipeline.as_ref().and_then(|p| p.hyde).unwrap_or(false)
        || resolved_config.pipeline.as_ref().and_then(|p| p.auto_translate).unwrap_or(false);

    if needs_llm && resolved_config.llm_config.is_none() {
        let models = crate::ai::models::scanner::scan_models();
        if let Some(llm_model) = models.iter().find(|m| matches!(m.kind, crate::ai::models::scanner::ModelKind::Llm)) {
            resolved_config.llm_config = Some(crate::ai::LlmConfig {
                backend: "local".to_string(),
                model: llm_model.id.clone(),
                api_key: None,
                base_url: None,
                system_prompt: None,
                max_tokens: Some(DEFAULT_LLM_MAX_TOKENS),
                temperature: None,
                response_format: Some("json".to_string()),
            });
        } else if let Ok(or_key) = std::env::var("OPENROUTER_API_KEY") {
            resolved_config.llm_config = Some(crate::ai::LlmConfig {
                backend: "openai".to_string(),
                model: FALLBACK_LLM_MODEL.to_string(),
                api_key: Some(or_key),
                base_url: Some(OPENROUTER_BASE_URL.to_string()),
                system_prompt: None,
                max_tokens: None,
                temperature: None,
                response_format: Some("json".to_string()),
            });
        }
    }

    resolved_config
}
