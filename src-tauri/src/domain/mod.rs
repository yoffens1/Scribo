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
pub mod section;
pub mod distribute;
pub mod tag;

pub use card::{Card, CardId, CardType, NewCard};
pub use fragment::{Fragment, FragmentId, NewFragment};
pub use note::{IndexingStatus, NewNote, Note, NoteId, NoteRevision, NoteLifecycle};
pub use schedule::{
    NewSchedule, Rating, ReviewLog, ReviewTarget, ReviewTargetType, Schedule, ScheduleId,
    SchedulerState, NoteDueCount, RepeatModeNode,
};
pub use search::{ScoredHit, SearchHit};
pub use section::{Section, SectionId, NewSection};
pub use distribute::{
    TopicChunk, RawBlock, CandidateNote, LlmRecommendation, DistributeAction,
    ChunkDistributionPlan, DraftDistributionPlan,
};
pub use tag::{Tag, TagId, NewTag, TagSource, NoteTagRelation, FragmentTagRelation};

/// Unix timestamp in seconds. Single shared alias to avoid `i64` everywhere.
pub type Timestamp = i64;
