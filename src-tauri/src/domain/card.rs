//! Card — an atomic unit of memorization (front/back pair).
//!
//! Cards are derived from Notes (manually or by an AI generator).
//! A single Note typically yields many Cards.
//!
//! Cards do NOT carry FSRS scheduling state — that lives in `Schedule`,
//! which references the card via a polymorphic target. This keeps the
//! same scheduling engine usable for both cards and whole-note reviews.

use serde::{Deserialize, Serialize};

use super::{FragmentId, NoteId, Timestamp};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CardId(pub i64);

impl From<i64> for CardId {
    fn from(v: i64) -> Self {
        Self(v)
    }
}

impl std::fmt::Display for CardId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// What kind of card this is. Drives how the UI renders it during review.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CardType {
    /// Plain front → back card (Anki "Basic").
    Basic,
    /// Bidirectional: prompts both front→back and back→front in alternation.
    Reverse,
    /// Cloze deletion: `front` contains the full text with `{{c1::...}}` markers,
    /// `back` is computed (or stores the answer text). Optional — implement when needed.
    Cloze,
}

impl CardType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Basic => "basic",
            Self::Reverse => "reverse",
            Self::Cloze => "cloze",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "basic" => Some(Self::Basic),
            "reverse" => Some(Self::Reverse),
            "cloze" => Some(Self::Cloze),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Card {
    pub id: CardId,
    pub note_id: Option<NoteId>,

    pub front: String,
    pub back: String,
    pub card_type: CardType,

    /// If created by AI from a specific fragment, link back so we can
    /// "show in context" or refresh when the source changes.
    pub source_fragment_id: Option<FragmentId>,
    pub source_offset: Option<i64>,
    pub source_length: Option<i64>,

    /// Provenance: "manual" | "ai:<model-name>" | "import:anki" | ...
    pub generated_by: Option<String>,

    pub is_suspended: bool,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

/// Input for inserting a new card. Repository assigns id/timestamps.
#[derive(Debug, Clone)]
pub struct NewCard {
    pub note_id: Option<NoteId>,
    pub front: String,
    pub back: String,
    pub card_type: CardType,
    pub source_fragment_id: Option<FragmentId>,
    pub source_offset: Option<i64>,
    pub source_length: Option<i64>,
    pub generated_by: Option<String>,
}


