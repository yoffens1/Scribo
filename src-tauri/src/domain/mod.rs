//! Domain layer — pure types describing the application's subject area.
//!
//! These types have ZERO dependencies on infrastructure:
//! no `sqlx`, no `reqwest`, no `tokio`, no `tauri`.
//! They can be used in any context (CLI, server, tests, mobile).
//!
//! Anything that needs a database or network goes into `db/` or `services/`.

pub mod card;
pub mod fragment;
pub mod note;
pub mod schedule;
pub mod search;

pub use card::{Card, CardId, CardType, NewCard};
pub use fragment::{Fragment, FragmentId, NewFragment};
pub use note::{IndexingStatus, NewNote, Note, NoteId, NoteRevision};
pub use schedule::{
    NewSchedule, Rating, ReviewLog, ReviewTarget, ReviewTargetType, Schedule, ScheduleId,
    SchedulerState,
};
pub use search::{ScoredHit, SearchHit};

/// Unix timestamp in seconds. Single shared alias to avoid `i64` everywhere.
pub type Timestamp = i64;
