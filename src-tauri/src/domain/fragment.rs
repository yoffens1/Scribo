//! Fragment — a fragment of a Note's text, used by the search/RAG pipeline.
//!
//! Fragments are an internal, technical concept: the user does not see them
//! directly. They store a slice of `note.content` together with its embedding
//! and an FTS index for hybrid search.

use serde::{Deserialize, Serialize};
use super::{NoteId, SectionId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FragmentId(pub i64);

impl From<i64> for FragmentId {
    fn from(v: i64) -> Self {
        Self(v)
    }
}

impl std::fmt::Display for FragmentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fragment {
    pub id: FragmentId,
    pub note_id: NoteId,
    pub section_id: Option<SectionId>,
    /// Sequential index of this fragment within its note (0-based).
    pub fragment_index: i64,
    pub text_clean: String,
    pub clean_hash: String,
    pub token_count: Option<i64>,
    /// Raw little-endian f32 vector. Decoded by the search service when needed.
    pub embedding: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FragmentInsertRow {
    pub fragment_index: i64,
    pub text_clean: String,
    pub clean_hash: String,
    pub token_count: Option<i64>,
    pub embedding: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct NewFragment {
    pub note_id: NoteId,
    pub section_id: Option<SectionId>,
    pub fragment_index: i64,
    pub text_clean: String,
    pub clean_hash: String,
    pub token_count: Option<i64>,
    pub embedding: Option<Vec<u8>>,
}
