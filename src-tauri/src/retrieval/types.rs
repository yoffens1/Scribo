//! # Retrieval Type Definitions
//!
//! Contains all shared types, enums and configuration structs used across the retrieval pipeline.
//! All public types are serialisable for Tauri IPC with camelCase field names.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::ai::LlmConfig;
use crate::domain::note::NoteId;

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

// ─── Result types ─────────────────────────────────────────────────────────────

/// Uniquely identifies a fragment by its parent note and its sequential index within that note.
/// Derives `Hash + Eq` so it can be used as a `HashMap` key in the RRF fusion step.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct FragmentRef {
    pub note_id: NoteId,
    pub fragment_index: i64,
}

/// Detailed breakdown of scores for a single search result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoreDebug {
    pub bm25_rank: Option<usize>,
    pub vector_rank: Option<usize>,
    pub rrf_score: f32,
    pub term_boost: f32,
    pub rerank_score: Option<f32>,
}

/// A single ranked search hit returned to the caller.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    /// Which fragment matched.
    pub fragment_ref: FragmentRef,
    /// Fused relevance score (higher is better). Semantics depend on the mode:
    /// pure keyword/vector — raw BM25/cosine; hybrid — RRF score; reranked — normalised LLM score.
    pub score: f32,
    /// Clean text of the fragment. Populated directly by SQL JOINs — no separate hydration needed.
    pub text: Option<String>,
    /// Title of the parent note.
    pub note_title: Option<String>,
    /// Detailed score breakdown, populated if `explain` option is true.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debug: Option<ScoreDebug>,
}

/// Post-retrieval filter criteria.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetrieveFilters {
    /// Restrict results to fragments belonging to this note only.
    pub note_id: Option<NoteId>,
}

/// Options controlling the retrieval call.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RetrieveOptions {
    /// Number of final results to return. Default: 5.
    pub top_k: Option<usize>,
    /// Optional post-retrieval filters.
    pub filters: Option<RetrieveFilters>,
    /// Restrict vector search to a specific chunk level.
    /// Level 1 = leaf fragments; higher levels = merged/grouped chunks.
    pub target_level: Option<i64>,
    /// Enable detailed scoring breakdown inside each SearchResult's debug field.
    pub explain: Option<bool>,
    /// Cut off any results below a percentage of the best result score.
    pub min_score_ratio: Option<f32>,
}

// ─── Fetch types (raw dump, not ranked) ──────────────────────────────────────

/// Input for the raw `fetch` operation — returns fragments without scoring or ranking.
/// Used by the indexing UI, sync, and admin tooling.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchQuery {
    /// Limit to fragments of a specific note. `None` returns across all notes.
    pub note_id: Option<NoteId>,
    /// Include soft-deleted fragments. Default: false.
    pub include_deleted: Option<bool>,
    /// Maximum number of results to return.
    pub limit: Option<usize>,
    /// Skip the first `offset` results (for pagination).
    pub offset: Option<usize>,
}

/// One row returned by `fetch`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchResult {
    /// Internal DB row ID of the fragment (chunk).
    pub fragment_id: Option<i64>,
    pub note_id: NoteId,
    /// Sequential position of this fragment within its note (0-based).
    pub fragment_index: i64,
    /// Cleaned and normalised fragment text.
    pub fragment_text: Option<String>,
    pub token_count: Option<i64>,
    /// Raw embedding bytes (f32 LE, ready for bytemuck cast).
    #[serde(with = "serde_bytes")]
    pub embedding: Vec<u8>,
}

// ─── Internal types (not exposed over IPC) ────────────────────────────────────

/// Indicates how a `QueryVariant` was produced during preprocessing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariantSource {
    /// The user's original query text.
    Original,
    /// A translation of the original query into the vault language.
    Translated,
    /// A synthetic document generated by HyDE to capture semantic context.
    Hyde,
    /// A term-level synonym of one of the other variants.
    Synonym,
}

/// One candidate query to be embedded and searched.
#[derive(Debug, Clone)]
pub struct QueryVariant {
    /// Actual query text to embed and/or search.
    pub text: String,
    /// ISO-639-1 language tag of `text`.
    pub lang: String,
    /// How this variant was produced (for logging/debugging).
    pub source: VariantSource,
    /// Fusion weight in the final RRF merge across variants.
    pub weight: f32,
    /// If `true`, this variant is skipped in FTS5/keyword search
    /// and only used for vector similarity (e.g. HyDE hypothetical documents).
    pub vector_only: bool,
}
