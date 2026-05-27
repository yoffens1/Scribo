//! Card — an atomic unit of memorization (front/back pair).
//!
//! Cards are derived from Sections of Notes (manually or by an AI generator).
//! A single Section typically yields one or more Cards.
//!
//! Cards do NOT carry FSRS scheduling state — that lives in `Schedule`,
//! which references the card via a polymorphic target. This keeps the
//! same scheduling engine usable for both cards and whole-note reviews.

use serde::{Deserialize, Serialize};
use super::{SectionId, NoteId, Timestamp};

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
    /// Front = heading of section, back = raw section body. Default for auto.
    Heading,
    /// Front/back override.
    Qa,
    /// Cloze deletion.
    Cloze,
    /// Manual front/back card.
    Manual,
}

impl CardType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Heading => "heading",
            Self::Qa => "qa",
            Self::Cloze => "cloze",
            Self::Manual => "manual",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "heading" => Some(Self::Heading),
            "qa" => Some(Self::Qa),
            "cloze" => Some(Self::Cloze),
            "manual" => Some(Self::Manual),
            _ => None,
        }
    }
}

impl std::fmt::Display for CardType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for CardType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or_else(|| format!("Unknown CardType: {}", s))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CardLifecycle {
    Fresh,
    Stale,
    Orphaned,
    Suspended,
}

impl CardLifecycle {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Fresh => "fresh",
            Self::Stale => "stale",
            Self::Orphaned => "orphaned",
            Self::Suspended => "suspended",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "fresh" => Some(Self::Fresh),
            "stale" => Some(Self::Stale),
            "orphaned" => Some(Self::Orphaned),
            "suspended" => Some(Self::Suspended),
            _ => None,
        }
    }
}

impl std::fmt::Display for CardLifecycle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for CardLifecycle {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or_else(|| format!("Unknown CardLifecycle: {}", s))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Card {
    pub id: CardId,
    pub note_id: NoteId,
    pub section_id: Option<SectionId>,
    pub card_type: CardType,
    pub custom_front: Option<String>,
    pub custom_back: Option<String>,
    pub cloze_mask: Option<String>,
    pub status: CardLifecycle,
    pub last_section_snapshot: Option<String>,
    pub generated_by: Option<String>,
    pub source_raw_hash_at_creation: Option<String>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

/// Ready for UI display: front and back text are fully resolved.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderedCard {
    pub card_id: CardId,
    pub front: String,
    pub back: String,
    pub card_type: CardType,
    // Context for UI
    pub note_id: NoteId,
    pub note_title: String,
    pub note_path: String,
}

impl Card {
    pub fn render(
        &self,
        section_opt: Option<&crate::domain::section::Section>,
        note_id: NoteId,
        note_title: String,
        note_path: String,
    ) -> RenderedCard {
        let section_text = match section_opt {
            Some(s) => &s.text_raw,
            None => self.last_section_snapshot.as_deref().unwrap_or(""),
        };
        let section_heading = match section_opt {
            Some(s) => s.heading.as_deref().unwrap_or("Untitled Section"),
            None => "Orphaned Section",
        };

        match self.card_type {
            CardType::Heading => {
                let default_front = section_heading.to_string();
                let default_back = section_text.to_string();
                RenderedCard {
                    card_id: self.id,
                    front: self.custom_front.clone().unwrap_or(default_front),
                    back: self.custom_back.clone().unwrap_or(default_back),
                    card_type: self.card_type,
                    note_id,
                    note_title,
                    note_path,
                }
            }
            CardType::Qa | CardType::Manual => RenderedCard {
                card_id: self.id,
                front: self.custom_front.clone().unwrap_or_default(),
                back: self.custom_back.clone().unwrap_or_default(),
                card_type: self.card_type,
                note_id,
                note_title,
                note_path,
            },
            CardType::Cloze => {
                let masked = apply_cloze_mask(section_text, self.cloze_mask.as_deref());
                RenderedCard {
                    card_id: self.id,
                    front: masked,
                    back: section_text.to_string(),
                    card_type: self.card_type,
                    note_id,
                    note_title,
                    note_path,
                }
            }
        }
    }
}

fn apply_cloze_mask(text: &str, mask_json: Option<&str>) -> String {
    let Some(mask) = mask_json else { return text.to_string(); };
    // TODO: implement cloze mask logic if/when needed
    let _ = mask;
    text.to_string()
}

#[derive(Debug, Clone)]
pub struct NewCard {
    pub note_id: NoteId,
    pub section_id: SectionId,
    pub card_type: CardType,
    pub custom_front: Option<String>,
    pub custom_back: Option<String>,
    pub cloze_mask: Option<String>,
    pub generated_by: Option<String>,
    pub source_raw_hash_at_creation: Option<String>,
}
