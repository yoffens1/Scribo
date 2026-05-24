//! Schedule — a polymorphic spaced-repetition record.
//!
//! A `Schedule` row represents "this thing should be reviewed on this date,
//! using these FSRS parameters". The `target` can be either a `Card` (Anki-style
//! flashcard review) OR a `Note` (whole-document refresher). The same FSRS
//! algorithm drives both — there is no separate "note review" engine.
//!
//! This is the key reason FSRS state was extracted out of `cards`: it lets
//! the Reviewer service operate uniformly on a single queue of due items.

use serde::{Deserialize, Serialize};

use super::{CardId, NoteId, Timestamp};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ScheduleId(pub i64);

impl From<i64> for ScheduleId {
    fn from(v: i64) -> Self {
        Self(v)
    }
}

/// What kind of entity this schedule reviews. Persisted as a TEXT column
/// alongside `target_id` (SQLite has no native polymorphic FK; integrity is
/// enforced at the application layer and via triggers).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReviewTargetType {
    Card,
    Note,
}

impl ReviewTargetType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Card => "card",
            Self::Note => "note",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "card" => Some(Self::Card),
            "note" => Some(Self::Note),
            _ => None,
        }
    }
}

/// A typed reference to either a Card or a Note. Use this in service APIs
/// instead of raw `(target_type, target_id)` tuples — it makes invalid
/// combinations unrepresentable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "id", rename_all = "lowercase")]
pub enum ReviewTarget {
    Card(CardId),
    Note(NoteId),
}

impl ReviewTarget {
    pub fn target_type(&self) -> ReviewTargetType {
        match self {
            Self::Card(_) => ReviewTargetType::Card,
            Self::Note(_) => ReviewTargetType::Note,
        }
    }

    pub fn target_id(&self) -> i64 {
        match self {
            Self::Card(c) => c.0,
            Self::Note(n) => n.0,
        }
    }

    pub fn from_parts(t: ReviewTargetType, id: i64) -> Self {
        match t {
            ReviewTargetType::Card => Self::Card(CardId(id)),
            ReviewTargetType::Note => Self::Note(NoteId(id)),
        }
    }
}

/// FSRS lifecycle state. Names follow the FSRS specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SchedulerState {
    /// Has never been reviewed.
    New,
    /// In the initial learning steps (short intervals).
    Learning,
    /// Stable review phase.
    Review,
    /// Lapsed from Review and being relearned.
    Relearning,
}

impl SchedulerState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Learning => "learning",
            Self::Review => "review",
            Self::Relearning => "relearning",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "new" => Some(Self::New),
            "learning" => Some(Self::Learning),
            "review" => Some(Self::Review),
            "relearning" => Some(Self::Relearning),
            _ => None,
        }
    }
}

/// User's rating of how well they recalled the item. Drives the FSRS update.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum Rating {
    /// Failed to recall — restart the card.
    Again = 1,
    /// Recalled with significant effort.
    Hard = 2,
    /// Recalled correctly with reasonable effort.
    Good = 3,
    /// Recalled effortlessly.
    Easy = 4,
}

impl Rating {
    pub fn as_i32(self) -> i32 {
        self as i32
    }

    pub fn from_i32(v: i32) -> Option<Self> {
        match v {
            1 => Some(Self::Again),
            2 => Some(Self::Hard),
            3 => Some(Self::Good),
            4 => Some(Self::Easy),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schedule {
    pub id: ScheduleId,
    pub target: ReviewTarget,

    pub state: SchedulerState,
    pub stability: f64,
    pub difficulty: f64,
    pub reps: i64,
    pub lapses: i64,

    pub last_reviewed: Option<Timestamp>,
    pub next_review: Option<Timestamp>,
}

#[derive(Debug, Clone)]
pub struct NewSchedule {
    pub target: ReviewTarget,
    /// When this item should first appear in the review queue.
    /// `None` = immediately (treated as "due now").
    pub initial_due: Option<Timestamp>,
}

/// One row in the audit log of past reviews. Used for analytics and FSRS
/// parameter optimization (re-fitting weights against the user's history).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewLog {
    pub log_id: i64,
    pub schedule_id: ScheduleId,
    pub rating: Rating,
    pub reviewed_at: Timestamp,
    /// Snapshot of stability BEFORE this review.
    pub prev_stability: Option<f64>,
    /// Snapshot of difficulty BEFORE this review.
    pub prev_difficulty: Option<f64>,
    /// Days since the previous review at the moment of this one.
    pub elapsed_days: Option<i64>,
}
