//! Section — a structural block of a note.
//!
//! A section represents a contiguous block of text (typically bounded by markdown headings).
//! It is used as the context boundary for flashcard generation and as a mean-pooled 
//! embedding target for the distribution pipeline.

use serde::{Deserialize, Serialize};
use super::NoteId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SectionId(pub i64);

impl From<i64> for SectionId {
    fn from(v: i64) -> Self {
        Self(v)
    }
}

impl std::fmt::Display for SectionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A section extracted from a note's AST. 
/// Tracks byte offsets in the raw note content for stable in-place editing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    /// Unique identifier for the section.
    pub id: SectionId,
    /// Parent note identifier.
    pub note_id: NoteId,
    /// Sequential index of this section within its note.
    pub section_index: i64,
    /// Raw un-cleaned markdown text content of this section.
    pub text_raw: String,
    /// Heading text if this section starts with a heading.
    pub heading: Option<String>,
    /// Heading level (1 to 6) if this section starts with a heading.
    pub heading_level: Option<i64>,
    /// BLAKE3 hash of the raw markdown text.
    pub raw_hash: String,
    /// BLAKE3 hash of the cleaned text.
    pub clean_hash: String,
    /// Absolute start offset in bytes in the parent note's markdown file.
    pub content_offset_start: i64,
    /// Absolute end offset in bytes in the parent note's markdown file.
    pub content_offset_end: i64,
}

/// Payload for creating a new section.
#[derive(Debug, Clone)]
pub struct NewSection {
    /// Parent note identifier.
    pub note_id: NoteId,
    /// Sequential index of this section within its note.
    pub section_index: i64,
    /// Raw un-cleaned markdown text content.
    pub text_raw: String,
    /// Optional heading text.
    pub heading: Option<String>,
    /// Optional heading level.
    pub heading_level: Option<i64>,
    /// BLAKE3 hash of the raw markdown.
    pub raw_hash: String,
    /// BLAKE3 hash of the cleaned text.
    pub clean_hash: String,
    /// Absolute start offset in bytes.
    pub content_offset_start: i64,
    /// Absolute end offset in bytes.
    pub content_offset_end: i64,
}
