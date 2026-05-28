//! # Reviewer Service
//!
//! Drives the spaced-repetition review loop using the **FSRS** (Free Spaced Repetition Scheduler)
//! algorithm from the `fsrs` crate.
//!
//! ## FSRS Overview
//!
//! FSRS models memory as a two-parameter state: `(stability, difficulty)`.
//! On each review the algorithm receives the current state, the number of days since the
//! last review, and the user's rating (`Again / Hard / Good / Easy`), then returns four
//! candidate next states — one per possible rating.  The chosen state's `interval` field
//! (in days) determines when the card should reappear.
//!
//! A desired retention rate of **90%** is used (`next_states(..., 0.90, ...)`).
//!
//! ## Workflow
//!
//! 1. [`get_due_reviews`](ReviewerService::get_due_reviews) — surfaces schedules where `next_review ≤ now`.
//! 2. [`rate_review`](ReviewerService::rate_review) — applies the FSRS update, logs the event, and reschedules.
//! 3. [`schedule_note_in_days`](ReviewerService::schedule_note_in_days) — manually pins a note's next review to a future date.

use chrono::Utc;
use fsrs::{FSRS, MemoryState};
use serde::{Deserialize, Serialize};
use rusqlite::Connection;

use crate::error::AppError;
use crate::domain::{NewSchedule, Rating, ReviewLog, Schedule, ScheduleId, SchedulerState, Timestamp};

/// The result returned to the caller after a successful review rating.
#[derive(Debug, Serialize, Deserialize)]
pub struct ReviewResult {
    /// Number of days until the next review (FSRS `interval`).
    pub scheduled_days: f32,
    /// Unix timestamp of the next scheduled review.
    pub next_review: Timestamp,
}

/// Stateless service — all state lives in the database.
pub struct ReviewerService;

impl ReviewerService {
    pub fn new() -> Self {
        Self
    }

    /// Returns up to `limit` schedules whose `next_review` timestamp is ≤ now.
    pub fn get_due_reviews(&self, conn: &Connection, limit: i64) -> Result<Vec<Schedule>, AppError> {
        let now = Utc::now().timestamp();
        crate::db::repos::schedules::find_due(conn, now, limit)
    }

    /// Applies a user rating to a schedule using FSRS and persists the updated state.
    ///
    /// ## Steps
    ///
    /// 1. Load the current schedule.
    /// 2. Compute `days_elapsed` since the last review.
    /// 3. Call `FSRS::next_states` to obtain candidate next states for all four ratings.
    /// 4. Select the state corresponding to `rating`.
    /// 5. Increment `reps`; increment `lapses` if rating is `Again`.
    /// 6. Write a [`ReviewLog`] entry.
    /// 7. Update the schedule with the new memory state and next review timestamp.
    pub fn rate_review(&self, conn: &Connection, schedule_id: ScheduleId, rating: Rating) -> Result<ReviewResult, AppError> {
        let now = Utc::now().timestamp();

        let schedule = crate::db::repos::schedules::find_by_id(conn, schedule_id)?
            .ok_or_else(|| AppError::Other("Schedule not found".to_string()))?;

        let fsrs = FSRS::new(Some(&fsrs::DEFAULT_PARAMETERS))
            .map_err(|e| AppError::Other(format!("FSRS error: {}", e)))?;

        // Days since last review (0 on first review — card was never seen before)
        let days_elapsed = if let Some(lr) = schedule.last_reviewed {
            let diff = now - lr;
            (diff / 86400).max(0) as u32
        } else {
            0
        };

        // New cards have no memory state yet — FSRS treats them as unseen.
        let current_memory_state = if schedule.state == SchedulerState::New {
            None
        } else {
            Some(MemoryState {
                stability: schedule.stability as f32,
                difficulty: schedule.difficulty as f32,
            })
        };

        let scheduled_cards = fsrs
            .next_states(current_memory_state, 0.90, days_elapsed)
            .map_err(|e| AppError::Other(format!("FSRS error: {}", e)))?;

        // Pick the branch corresponding to the user's rating
        let new_state = match rating {
            Rating::Again => scheduled_cards.again,
            Rating::Hard  => scheduled_cards.hard,
            Rating::Good  => scheduled_cards.good,
            Rating::Easy  => scheduled_cards.easy,
        };

        let reps = schedule.reps + 1;
        let mut lapses = schedule.lapses;
        if rating == Rating::Again {
            lapses += 1; // "Again" counts as a lapse — card forgotten
        }

        let scheduled_days = new_state.interval;
        let next_review = now + (scheduled_days as f64 * 86400.0) as i64;
        let updated_state = if schedule.state == SchedulerState::New {
            SchedulerState::Learning // First review moves card from New → Learning
        } else {
            SchedulerState::Review
        };

        // 1. Log the review for analytics / retention tracking
        let log = ReviewLog {
            log_id: 0, // DB auto-increments
            schedule_id,
            rating,
            reviewed_at: now,
            prev_stability: Some(schedule.stability),
            prev_difficulty: Some(schedule.difficulty),
            elapsed_days: Some(days_elapsed as i64),
        };
        crate::db::repos::review_logs::insert(conn, &log)?;

        // 2. Persist the updated FSRS memory state and next review date
        crate::db::repos::schedules::update_state(
            conn,
            schedule_id,
            updated_state,
            new_state.memory.stability as f64,
            new_state.memory.difficulty as f64,
            reps,
            lapses,
            Some(now),
            Some(next_review),
        )?;

        Ok(ReviewResult {
            scheduled_days,
            next_review,
        })
    }

    /// Manually overrides when a note should next appear for review.
    ///
    /// If no schedule exists for the note, creates one with `state = New`.
    /// If one already exists, only updates `next_review`.
    pub fn schedule_note_in_days(&self, conn: &Connection, note_id: crate::domain::NoteId, days: i64) -> Result<Schedule, AppError> {
        let now = Utc::now().timestamp();
        let due_time = now + days * 86400;

        let target = crate::domain::ReviewTarget::Note(note_id);
        let existing = crate::db::repos::schedules::find_by_target(conn, target)?;

        if let Some(mut schedule) = existing {
            schedule.next_review = Some(due_time);
            crate::db::repos::schedules::set_next_review(conn, schedule.id, Some(due_time))?;
            Ok(schedule)
        } else {
            let new_schedule = NewSchedule {
                target,
                initial_due: Some(due_time),
            };
            let schedule_id = crate::db::repos::schedules::insert(conn, new_schedule)?;

            let schedule = Schedule {
                id: schedule_id,
                target,
                state: SchedulerState::New,
                stability: 0.0,
                difficulty: 0.0,
                reps: 0,
                lapses: 0,
                last_reviewed: None,
                next_review: Some(due_time),
            };
            Ok(schedule)
        }
    }

    /// Returns per-note due counts aggregated in a hierarchy (for the tree-view UI component).
    pub fn get_hierarchical_due_counts(&self, conn: &Connection) -> Result<Vec<crate::domain::NoteDueCount>, AppError> {
        let now = Utc::now().timestamp();
        crate::db::repos::schedules::get_hierarchical_due_counts(conn, now)
    }

    /// Returns the repeat-mode tree used by the review session UI.
    pub fn get_repeat_mode_tree(&self, conn: &Connection) -> Result<Vec<crate::domain::RepeatModeNode>, AppError> {
        let now = Utc::now().timestamp();
        crate::db::repos::schedules::get_repeat_mode_tree(conn, now)
    }
}
