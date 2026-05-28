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

/// Represents a technical chunk of text within a Note, optimized for embedding and vector search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fragment {
    /// Unique identifier for this fragment.
    pub id: FragmentId,
    /// Parent note identifier.
    pub note_id: NoteId,
    /// Reference to the note section this fragment is part of.
    pub section_id: Option<SectionId>,
    /// Sequential index of this fragment within its note (0-based).
    pub fragment_index: i64,
    /// Cleaned text content of the fragment (without markdown syntax/formatting).
    pub text_clean: String,
    /// BLAKE3 hash of the cleaned text.
    pub clean_hash: String,
    /// Token count estimated using tiktoken, if calculated.
    pub token_count: Option<i64>,
    /// Raw byte vector containing the little-endian f32 embedding.
    pub embedding: Option<Vec<u8>>,
}

/// Payload for inserting a new fragment during the indexing process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FragmentInsertRow {
    /// Sequential index of this fragment within its note.
    pub fragment_index: i64,
    /// Cleaned text content of the fragment.
    pub text_clean: String,
    /// BLAKE3 hash of the cleaned text.
    pub clean_hash: String,
    /// Estimated token count.
    pub token_count: Option<i64>,
    /// Raw byte representation of the f32 vector embedding.
    pub embedding: Vec<u8>,
}

/// Payload for creating a new fragment before database assignment.
#[derive(Debug, Clone)]
pub struct NewFragment {
    /// Parent note identifier.
    pub note_id: NoteId,
    /// Reference to the note section.
    pub section_id: Option<SectionId>,
    /// Sequential index within the note.
    pub fragment_index: i64,
    /// Cleaned text content.
    pub text_clean: String,
    /// BLAKE3 hash of the cleaned text.
    pub clean_hash: String,
    /// Estimated token count.
    pub token_count: Option<i64>,
    /// Optional raw byte vector embedding.
    pub embedding: Option<Vec<u8>>,
}
