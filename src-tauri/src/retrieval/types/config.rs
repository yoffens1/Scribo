//! # Retrieval Configuration Types
//!
//! Top-level configuration structs that control how the retrieval pipeline behaves.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::ai::LlmConfig;

// ─── Search mode ─────────────────────────────────────────────────────────────

/// How the pipeline should search fragments.
///
/// - `Embedding` — pure vector cosine-similarity scan.
/// - `Keyword`   — BM25/FTS5 full-text search.
/// - `Hybrid`    — runs both branches and merges via RRF.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RetrievalMode {
    Embedding,
    Keyword,
    Hybrid,
}

// ─── Synonym expansion mode ───────────────────────────────────────────────────

/// Controls how query term expansion is performed.
///
/// - `Off`    — no expansion (default).
/// - `Static` — dictionary lookup from `data/synonyms.json`.
/// - `Llm`    — ask the LLM to generate up to 3 query synonyms.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SynonymExpansion {
    Off,
    Static,
    Llm,
}

impl Default for SynonymExpansion {
    fn default() -> Self {
        SynonymExpansion::Off
    }
}

// ─── Rerank mode ──────────────────────────────────────────────────────────────

/// LLM reranking strategy applied after fusion.
///
/// - `Scoring`  — the LLM assigns a numeric relevance score to every candidate.
/// - `Listwise` — the LLM returns a reordered index list of candidates.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RerankMode {
    Scoring,
    Listwise,
}

// ─── Pipeline configuration ───────────────────────────────────────────────────

/// Query preprocessing knobs.
/// All fields are optional so the front-end can selectively enable features.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineConfig {
    /// Translate the query to the vault language when they differ.
    pub auto_translate: Option<bool>,
    /// Synonym expansion strategy.
    pub expand_synonyms: Option<SynonymExpansion>,
    /// Custom synonym dictionary (overrides the built-in `data/synonyms.json`).
    pub synonym_dict: Option<HashMap<String, Vec<String>>>,
    /// Enable HyDE (Hypothetical Document Embeddings).
    pub hyde: Option<bool>,
}

// ─── AI reranking configuration ───────────────────────────────────────────────

/// Controls post-fusion LLM reranking.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiRerankConfig {
    /// Whether reranking is active for this request.
    pub enabled: bool,
    /// Which reranking algorithm to use. Defaults to `Scoring`.
    pub mode: Option<RerankMode>,
    /// Maximum number of candidates passed to the LLM. Defaults to 25.
    pub max_candidates: Option<usize>,
}

// ─── Adaptive weights ─────────────────────────────────────────────────────────

/// Per-language weight multipliers applied to query variants.
///
/// Queries whose language matches the vault get a boost; cross-language variants
/// are down-weighted so they don't dominate the fusion.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdaptiveWeights {
    /// Multiplier applied when the variant's language matches the vault. Default: 1.5
    pub same_lang: f32,
    /// Multiplier applied when the variant's language differs from the vault. Default: 0.5
    pub other_lang: f32,
}

impl Default for AdaptiveWeights {
    fn default() -> Self {
        Self {
            same_lang: 1.5,
            other_lang: 0.5,
        }
    }
}

// ─── Tuning parameters ────────────────────────────────────────────────────────

/// All "magic numbers" of the retrieval pipeline in one place.
/// Front-end or tests can override specific values without recompiling.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RetrievalTuning {
    /// How many extra results to fetch per variant before final truncation.
    /// `over_fetch = top_k * over_fetch_multiplier`. Default: 3.
    pub over_fetch_multiplier: usize,
    /// Hard cap on the internal over-fetch count regardless of `top_k`. Default: 50.
    pub over_fetch_cap: usize,
    /// RRF smoothing constant `k`. Higher values reduce the impact of top ranks.
    /// Empirically 60 works well for most corpora. Default: None (which falls back to 60.0 or DB).
    pub rrf_k: Option<f32>,
    /// Pool size multiplier for reranking (`candidates = top_k * multiplier`). Default: 4.
    pub rerank_pool_multiplier: usize,
    /// Relevance weight for HyDE variants in the final RRF fusion. Default: 0.8.
    pub hyde_weight: f32,
    /// Relevance weight for synonym-expanded variants in the final RRF fusion. Default: 0.6.
    pub synonym_weight: f32,
    /// Denominator used to normalise raw LLM scoring reranker scores to [0, 1].
    /// Should match the maximum score the LLM prompt requests. Default: 10.0.
    pub scoring_max_score: f32,
}

impl Default for RetrievalTuning {
    fn default() -> Self {
        Self {
            over_fetch_multiplier: 3,
            over_fetch_cap: 50,
            rrf_k: None,
            rerank_pool_multiplier: 4,
            hyde_weight: 0.8,
            synonym_weight: 0.6,
            scoring_max_score: 10.0,
        }
    }
}

// ─── Request configuration ────────────────────────────────────────────────────

/// Top-level retrieval request configuration sent by the front-end.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetrievalConfig {
    /// Which search strategy to use.
    pub mode: RetrievalMode,
    /// Weight of the vector branch relative to the keyword branch in hybrid mode.
    /// Passed as the RRF list weight. Default (when `None`): 1.0.
    pub embedding_weight: Option<f32>,
    /// Optional query preprocessing stage toggles.
    pub pipeline: Option<PipelineConfig>,
    /// Optional LLM reranking configuration.
    pub ai_rerank: Option<AiRerankConfig>,
    /// Override the vault language instead of auto-detecting it from stored fragments.
    pub vault_lang: Option<String>,
    /// LLM connection config for preprocessing (translation, HyDE, synonyms, reranking).
    /// When `None`, the pipeline falls back to the app-level cached `LlmService`.
    pub llm_config: Option<LlmConfig>,
    /// Per-language variant weight multipliers.
    pub adaptive_weights: Option<AdaptiveWeights>,
    /// Fine-grained numeric tuning. Falls back to `RetrievalTuning::default()` when `None`.
    pub tuning: Option<RetrievalTuning>,
}
