//! Search results — domain types for retrieval output.

use serde::{Deserialize, Serialize};

use super::{FragmentId, NoteId};

/// A single hit returned by retrieval (vector / FTS / hybrid).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub fragment_id: FragmentId,
    pub note_id: NoteId,
    pub fragment_index: i64,
    pub text: String,
    /// Title of the parent note, denormalized for convenient UI rendering.
    pub note_title: Option<String>,
    pub note_file_path: Option<String>,
    pub snippet: Option<String>,
}

/// A search hit annotated with a score (cosine similarity, BM25, RRF, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredHit {
    pub hit: SearchHit,
    pub score: f32,
}
