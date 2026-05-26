//! Note — a user-authored document. The "macro" unit of knowledge.
//!
//! Notes are the source of truth. They live primarily in the database
//! (`content` column). A note MAY be exported to a Markdown file via
//! `file_path` for compatibility with external tools (git, Obsidian, etc.),
//! but the database is authoritative — file is just a projection.

use serde::{Deserialize, Serialize};

use super::Timestamp;

/// Strongly-typed note identifier. Prevents accidental id mixing
/// (e.g. passing a CardId where NoteId is expected).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NoteId(pub i64);

impl From<i64> for NoteId {
    fn from(v: i64) -> Self {
        Self(v)
    }
}

impl std::fmt::Display for NoteId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Status of background indexing (fragmenting + embedding).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IndexingStatus {
    /// Note exists but indexing has not started.
    Pending,
    /// Indexing is currently running.
    Indexing,
    /// Successfully indexed; fragments and embeddings are up to date.
    Indexed,
    /// Indexing failed; see `indexing_error` on the Note.
    Failed,
    /// Note was modified after the last successful indexing.
    Stale,
}

impl IndexingStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Indexing => "indexing",
            Self::Indexed => "indexed",
            Self::Failed => "failed",
            Self::Stale => "stale",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "indexing" => Some(Self::Indexing),
            "indexed" => Some(Self::Indexed),
            "failed" => Some(Self::Failed),
            "stale" => Some(Self::Stale),
            _ => None,
        }
    }
}

/// A user-authored document.
///
/// `content` is the primary storage. `file_path` is optional and only
/// set when the user wants the note mirrored on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub id: NoteId,
    pub title: String,
    pub content: String,
    /// Tags as a JSON-encoded array of strings (kept as a string for simple
    /// SQLite storage; parsed by the repository if needed).
    pub tags: Option<String>,

    /// File mirroring (optional). When `None`, the note lives only in the database.
    pub file_path: Option<String>,
    pub file_name: Option<String>,
    pub file_hash: Option<String>,
    pub file_mtime: Option<Timestamp>,

    /// Indexing state for the search/RAG pipeline.
    pub indexing_status: IndexingStatus,
    pub indexing_error: Option<String>,
    pub indexed_at: Option<Timestamp>,
    pub embedding_model: Option<String>,
    pub embedding_dimension: Option<i64>,
    pub indexing_version: Option<String>,

    pub is_archived: bool,
    pub is_deleted: bool,

    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

/// Input for creating a new note. The repository assigns the id and timestamps.
#[derive(Debug, Clone)]
pub struct NewNote {
    pub title: String,
    pub content: String,
    pub tags: Option<String>,
    pub file_path: Option<String>,
}

/// A historical revision of a note's content (stored as a diffy patch
/// against the previous revision).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteRevision {
    pub revision_id: i64,
    pub note_id: NoteId,
    /// diffy-formatted patch transforming previous content into this one.
    pub patch: String,
    pub created_at: Timestamp,
}

