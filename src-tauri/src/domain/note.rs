//! Note — a user-authored document. The "macro" unit of knowledge.
//!
//! Заметка — единица знания. БД — единственный источник истины.

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
    Pending,    // создана, ждёт индексации
    Indexing,   // в процессе
    Indexed,    // готова к поиску/повторению
    Failed,     // упало с ошибкой
    Stale,      // контент изменился, нужна переиндексация
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

/// Заметка — единица знания. БД — единственный источник истины.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub id: NoteId,

    // Контент — то что видит и редактирует юзер.
    pub title: String,
    pub content: String,           // raw markdown
    pub content_hash: String,      // blake3 от нормализованного content
    pub tags: Option<String>,

    // Иерархия
    pub parent_note_id: Option<NoteId>,
    pub path_cached: String,
    pub sort_order: i64,
    pub icon: Option<String>,

    // Состояние индексирования (для search и cards).
    pub indexing_status: IndexingStatus,
    pub indexing_error: Option<String>,
    pub indexed_at: Option<Timestamp>,
    pub embedding_model: Option<String>,
    pub embedding_dimension: Option<i64>,
    pub indexing_version: Option<String>,

    // Жизненный цикл.
    pub is_draft: bool,
    pub is_archived: bool,
    pub is_deleted: bool,          // soft delete для синхронизации
    pub is_pinned: bool,
    pub is_favorite: bool,

    // Обучение
    pub mastery: Option<f32>,
    pub last_studied: Option<Timestamp>,

    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

/// Input for creating a new note. The repository assigns the id and timestamps.
#[derive(Debug, Clone, Default)]
pub struct NewNote {
    pub title: String,
    pub content: String,
    pub tags: Option<String>,
    pub parent_note_id: Option<NoteId>,
    pub path_cached: Option<String>,
    pub sort_order: Option<i64>,
    pub icon: Option<String>,
    pub is_draft: bool,
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
