//! Search results — domain types for retrieval output.

use serde::{Deserialize, Serialize};

use super::{FragmentId, NoteId};

/// A single hit returned by retrieval (vector / FTS / hybrid).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    /// Identifier of the fragment.
    pub fragment_id: FragmentId,
    /// Identifier of the parent note containing this fragment.
    pub note_id: NoteId,
    /// Sequential index of the fragment within its note.
    pub fragment_index: i64,
    /// Cleaned text content of the fragment.
    pub text: String,
    /// Title of the parent note, denormalized for convenient UI rendering.
    pub note_title: Option<String>,
    /// Materialized path of the parent note, for display context.
    pub note_path: Option<String>,
    /// Highlighting snippet, if available.
    pub snippet: Option<String>,
}

/// A search hit annotated with a score (cosine similarity, BM25, RRF, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredHit {
    /// The search hit detail.
    pub hit: SearchHit,
    /// The computed ranking/matching score.
    pub score: f32,
}
