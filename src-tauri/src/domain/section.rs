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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    pub id: SectionId,
    pub note_id: NoteId,
    pub section_index: i64,
    pub text_raw: String,
    pub heading: Option<String>,
    pub heading_level: Option<i64>,
    pub raw_hash: String,
    pub clean_hash: String,
    pub content_offset_start: i64,
    pub content_offset_end: i64,
}

#[derive(Debug, Clone)]
pub struct NewSection {
    pub note_id: NoteId,
    pub section_index: i64,
    pub text_raw: String,
    pub heading: Option<String>,
    pub heading_level: Option<i64>,
    pub raw_hash: String,
    pub clean_hash: String,
    pub content_offset_start: i64,
    pub content_offset_end: i64,
}
