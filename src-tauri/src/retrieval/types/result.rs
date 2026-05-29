//! # Retrieval Result Types
//!
//! Public output types returned by the retrieval pipeline and exposed over Tauri IPC.

use serde::{Deserialize, Serialize};
use crate::domain::note::NoteId;

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
