//! Repository traits.
//!
//! Services depend on these abstractions, NOT on a concrete database
//! implementation. Your existing rusqlite/sqlx code implements these traits
//! in `db/repos/<concrete>.rs`. This decoupling is what makes services
//! testable (you can pass a mock in tests) and lets you swap the backend
//! later without touching service logic.

pub mod cards;
pub mod fragments;
pub mod notes;
pub mod review_logs;
pub mod schedules;

use async_trait::async_trait;

use crate::domain::{
    Card, CardId, NewCard, NewSchedule, Note, NoteId, ReviewLog, ReviewTarget, Schedule,
    ScheduleId, SchedulerState, Timestamp,
};

#[derive(Debug, thiserror::Error)]
pub enum RepoError {
    #[error("database error: {0}")]
    Db(String),
    #[error("not found")]
    NotFound,
    #[error("constraint violation: {0}")]
    Constraint(String),
}

#[async_trait]
pub trait NotesRepo: Send + Sync {
    async fn find_by_id(&self, id: NoteId) -> Result<Option<Note>, RepoError>;
}

#[async_trait]
pub trait CardsRepo: Send + Sync {
    async fn find_by_id(&self, id: CardId) -> Result<Option<Card>, RepoError>;
    async fn insert(&self, new: NewCard) -> Result<CardId, RepoError>;
    async fn list_by_note(&self, note_id: NoteId) -> Result<Vec<Card>, RepoError>;
    async fn delete(&self, id: CardId) -> Result<(), RepoError>;
}

#[async_trait]
pub trait SchedulesRepo: Send + Sync {
    async fn find_by_id(&self, id: ScheduleId) -> Result<Option<Schedule>, RepoError>;
    async fn find_by_target(&self, target: ReviewTarget) -> Result<Option<Schedule>, RepoError>;
    /// Items whose `next_review` is on or before `now`, ordered ascending.
    /// `None` next_review (suspended) are excluded.
    async fn find_due(&self, now: Timestamp, limit: i64) -> Result<Vec<Schedule>, RepoError>;
    async fn count_due(&self, now: Timestamp) -> Result<i64, RepoError>;
    async fn insert(&self, new: NewSchedule) -> Result<ScheduleId, RepoError>;
    async fn update_state(
        &self,
        id: ScheduleId,
        state: SchedulerState,
        stability: f64,
        difficulty: f64,
        reps: i64,
        lapses: i64,
        last_reviewed: Option<Timestamp>,
        next_review: Option<Timestamp>,
    ) -> Result<(), RepoError>;
    async fn set_next_review(
        &self,
        id: ScheduleId,
        next_review: Option<Timestamp>,
    ) -> Result<(), RepoError>;
    async fn delete_by_target(&self, target: ReviewTarget) -> Result<(), RepoError>;
}

#[async_trait]
pub trait ReviewLogsRepo: Send + Sync {
    async fn insert(&self, log: &ReviewLog) -> Result<i64, RepoError>;
    async fn list_for_schedule(&self, id: ScheduleId) -> Result<Vec<ReviewLog>, RepoError>;
}
