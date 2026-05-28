//! Note — a user-authored document. The "macro" unit of knowledge.

use serde::{Deserialize, Serialize};
use super::Timestamp;

/// Strongly-typed note identifier. Prevents accidental id mixing.
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
    /// Newly created, waiting to be indexed.
    Pending,
    /// Currently being processed (chunking or embedding).
    Indexing,
    /// Successfully indexed and available for search/retrieval.
    Indexed,
    /// Indexing failed with an error.
    Failed,
    /// Note content has changed, needs re-indexing.
    Stale,
}

impl IndexingStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending  => "pending",
            Self::Indexing => "indexing",
            Self::Indexed  => "indexed",
            Self::Failed   => "failed",
            Self::Stale    => "stale",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "pending"  => Self::Pending,
            "indexing" => Self::Indexing,
            "indexed"  => Self::Indexed,
            "failed"   => Self::Failed,
            "stale"    => Self::Stale,
            _ => return None,
        })
    }
}

impl std::fmt::Display for IndexingStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for IndexingStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or_else(|| format!("Unknown IndexingStatus: {}", s))
    }
}

/// The lifecycle status of a Note.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NoteLifecycle {
    /// Work-in-progress draft, not indexed.
    Draft,
    /// Regular note.
    Active,
    /// Archived note, hidden from normal views and not indexed.
    Archived,
    /// Soft-deleted note, retained for synchronization, removed by GC.
    Deleted,
}

impl NoteLifecycle {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Active => "active",
            Self::Archived => "archived",
            Self::Deleted => "deleted",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "draft" => Some(Self::Draft),
            "active" => Some(Self::Active),
            "archived" => Some(Self::Archived),
            "deleted" => Some(Self::Deleted),
            _ => None,
        }
    }
}

impl std::fmt::Display for NoteLifecycle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for NoteLifecycle {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or_else(|| format!("Unknown NoteLifecycle: {}", s))
    }
}

/// A Note represents a user-authored document and is the core unit of knowledge.
/// The database is the single source of truth for all note content and state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    /// Unique identifier for the note.
    pub id: NoteId,

    // Content: what the user sees and edits.
    /// Title of the note.
    pub title: String,
    /// Raw markdown content of the note.
    pub content: String,
    /// BLAKE3 hash of the normalized note content, used for change detection.
    pub content_hash: String,

    // Hierarchy
    /// Optional parent note id to support hierarchical folder-like structures.
    pub parent_note_id: Option<NoteId>,
    /// Materialized/cached path from the root note down to this note.
    pub path_cached: String,
    /// Order for sorting siblings at the same hierarchical level.
    pub sort_order: i64,
    /// Optional icon name/emoji for sidebar/UI rendering.
    pub icon: Option<String>,

    // Indexing status (for search, RAG, and SRS card generation).
    /// Current background indexing status.
    pub indexing_status: IndexingStatus,
    /// Description of the indexing error, if status is Failed.
    pub indexing_error: Option<String>,
    /// Time when the note was last successfully indexed.
    pub indexed_at: Option<Timestamp>,
    /// Name of the model used to generate embeddings.
    pub embedding_model: Option<String>,
    /// Vector dimensions of the generated embeddings.
    pub embedding_dimension: Option<i64>,
    /// Version of the indexing pipeline schema/logic.
    pub indexing_version: Option<String>,

    // Lifecycle
    /// Current state in the note lifecycle.
    pub lifecycle: NoteLifecycle,
    /// Whether the note is pinned at the top of lists.
    pub is_pinned: bool,
    /// Whether the note is marked as a user favorite.
    pub is_favorite: bool,

    // Study metadata
    /// Overall mastery level (e.g. calculated from SRS card performance).
    pub mastery: Option<f32>,
    /// When the note or its child cards were last reviewed.
    pub last_studied: Option<Timestamp>,

    /// Creation timestamp in UTC seconds.
    pub created_at: Timestamp,
    /// Last update timestamp in UTC seconds.
    pub updated_at: Timestamp,
}

impl Note {
    pub fn new(
        id: NoteId,
        title: String,
        content: String,
        parent_note_id: Option<NoteId>,
        path_cached: String,
        sort_order: i64,
        icon: Option<String>,
        lifecycle: NoteLifecycle,
        is_pinned: bool,
        is_favorite: bool,
        created_at: Timestamp,
    ) -> Self {
        let content_hash = blake3::hash(content.as_bytes()).to_hex().to_string();
        Self {
            id,
            title,
            content,
            content_hash,
            parent_note_id,
            path_cached,
            sort_order,
            icon,
            indexing_status: IndexingStatus::Pending,
            indexing_error: None,
            indexed_at: None,
            embedding_model: None,
            embedding_dimension: None,
            indexing_version: None,
            lifecycle,
            is_pinned,
            is_favorite,
            mastery: None,
            last_studied: None,
            created_at,
            updated_at: created_at,
        }
    }

    pub fn update_content(&mut self, new_content: String, updated_at: Timestamp) {
        let new_hash = blake3::hash(new_content.as_bytes()).to_hex().to_string();
        if self.content_hash != new_hash {
            self.content = new_content;
            self.content_hash = new_hash;
            self.indexing_status = IndexingStatus::Stale;
        }
        self.updated_at = updated_at;
    }

    /// Returns true if the note is eligible to be processed by the search, RAG, and card-generation pipelines.
    pub fn is_indexable(&self) -> bool {
        self.lifecycle == NoteLifecycle::Active
    }

    /// Returns true if the note should be displayed in the SRS/repeat-mode study tree.
    pub fn is_visible_in_tree(&self) -> bool {
        self.lifecycle != NoteLifecycle::Deleted && self.lifecycle != NoteLifecycle::Draft
    }

    /// Returns true if the note is a work-in-progress draft that can be distributed to other notes.
    pub fn is_distributable(&self) -> bool {
        self.lifecycle == NoteLifecycle::Draft
    }

    pub fn summary(&self) -> NoteSummary {
        NoteSummary {
            id: self.id,
            title: self.title.clone(),
            icon: self.icon.clone(),
            parent_note_id: self.parent_note_id,
            path_cached: self.path_cached.clone(),
            lifecycle: self.lifecycle,
            is_pinned: self.is_pinned,
            mastery: self.mastery,
            last_studied: self.last_studied,
            updated_at: self.updated_at,
        }
    }
}

/// What the UI sees in lists/trees. No heavy fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteSummary {
    pub id: NoteId,
    pub title: String,
    pub icon: Option<String>,
    pub parent_note_id: Option<NoteId>,
    pub path_cached: String,
    pub lifecycle: NoteLifecycle,
    pub is_pinned: bool,
    pub mastery: Option<f32>,
    pub last_studied: Option<Timestamp>,
    pub updated_at: Timestamp,
}

/// Input for creating a new note. The repository assigns the id and timestamps.
#[derive(Debug, Clone, Default)]
pub struct NewNote {
    pub title: String,
    pub content: String,
    pub parent_note_id: Option<NoteId>,
    pub path_cached: Option<String>,
    pub sort_order: Option<i64>,
    pub icon: Option<String>,
    pub lifecycle: Option<NoteLifecycle>,
    pub is_pinned: bool,
    pub is_favorite: bool,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexing_status_serde_matches_as_str() {
        for s in [
            IndexingStatus::Pending,
            IndexingStatus::Indexing,
            IndexingStatus::Indexed,
            IndexingStatus::Failed,
            IndexingStatus::Stale,
        ] {
            let json = serde_json::to_string(&s).unwrap();
            let stripped = json.trim_matches('"');
            assert_eq!(stripped, s.as_str());
            assert_eq!(IndexingStatus::parse(s.as_str()), Some(s));
        }
    }

    #[test]
    fn note_lifecycle_serde() {
        for l in [
            NoteLifecycle::Draft,
            NoteLifecycle::Active,
            NoteLifecycle::Archived,
            NoteLifecycle::Deleted,
        ] {
            let json = serde_json::to_string(&l).unwrap();
            let stripped = json.trim_matches('"');
            assert_eq!(stripped, l.as_str());
            assert_eq!(NoteLifecycle::parse(l.as_str()), Some(l));
        }
    }
}
