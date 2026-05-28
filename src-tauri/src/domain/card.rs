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

/// Represents an atomic flashcard derived from a specific note section.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Card {
    /// Unique identifier for this card.
    pub id: CardId,
    /// Identifier of the parent note this card belongs to.
    pub note_id: NoteId,
    /// Reference to the specific note section this card is associated with.
    /// Can be `None` if the section is orphaned/deleted.
    pub section_id: Option<SectionId>,
    /// The type of card (e.g. heading-based, Q&A, Cloze deletion).
    pub card_type: CardType,
    /// Optional customized front text that overrides the default heading.
    pub custom_front: Option<String>,
    /// Optional customized back text that overrides the default body.
    pub custom_back: Option<String>,
    /// Optional configuration for Cloze deletion masks.
    pub cloze_mask: Option<String>,
    /// Current lifecycle status (fresh, stale, orphaned, suspended).
    pub status: CardLifecycle,
    /// Cached copy of the section text at the time of card generation, used as fallback.
    pub last_section_snapshot: Option<String>,
    /// Name/ident of the generator or service that created this card (e.g. "ai").
    pub generated_by: Option<String>,
    /// Hash of the source section content when this card was last updated/created,
    /// used to detect if the section content changed.
    pub source_raw_hash_at_creation: Option<String>,
    /// Creation timestamp in UTC seconds.
    pub created_at: Timestamp,
    /// Last update timestamp in UTC seconds.
    pub updated_at: Timestamp,
}

/// Ready for UI display: front and back text are fully resolved.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderedCard {
    /// Identifier of the rendered card.
    pub card_id: CardId,
    /// Resolved front-side content of the card (e.g. question or heading).
    pub front: String,
    /// Resolved back-side content of the card (e.g. answer or body).
    pub back: String,
    /// Type of the card, determining its UI layout.
    pub card_type: CardType,
    // Context for UI
    /// Identifier of the parent note.
    pub note_id: NoteId,
    /// Title of the parent note, for display context.
    pub note_title: String,
    /// Cached path of the parent note, for display context.
    pub note_path: String,
}

impl Card {
    /// Renders the card into a `RenderedCard` by combining its metadata and optional current section text.
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

/// Payload for creating a new card.
#[derive(Debug, Clone)]
pub struct NewCard {
    /// Note this card is created for.
    pub note_id: NoteId,
    /// Section this card is linked to.
    pub section_id: SectionId,
    /// Type of card to generate.
    pub card_type: CardType,
    /// Custom front override, if any.
    pub custom_front: Option<String>,
    /// Custom back override, if any.
    pub custom_back: Option<String>,
    /// Cloze deletion mask configuration, if any.
    pub cloze_mask: Option<String>,
    /// Name of the generator that created this card.
    pub generated_by: Option<String>,
    /// Hash of the source section when created, to detect future changes.
    pub source_raw_hash_at_creation: Option<String>,
}
