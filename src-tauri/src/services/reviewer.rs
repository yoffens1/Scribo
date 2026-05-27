use chrono::Utc;
use fsrs::{FSRS, MemoryState};
use serde::{Deserialize, Serialize};
use rusqlite::Connection;

use crate::error::AppError;
use crate::domain::{NewSchedule, Rating, ReviewLog, Schedule, ScheduleId, SchedulerState, Timestamp};

#[derive(Debug, Serialize, Deserialize)]
pub struct ReviewResult {
    pub scheduled_days: f32,
    pub next_review: Timestamp,
}

pub struct ReviewerService;

impl ReviewerService {
    pub fn new() -> Self {
        Self
    }

    /// Fetches a list of schedules that are currently due for review.
    pub fn get_due_reviews(&self, conn: &Connection, limit: i64) -> Result<Vec<Schedule>, AppError> {
        let now = Utc::now().timestamp();
        crate::db::repos::schedules::find_due(conn, now, limit)
    }

    /// Rates a review using the FSRS algorithm, updates the schedule, and logs the review.
    pub fn rate_review(&self, conn: &Connection, schedule_id: ScheduleId, rating: Rating) -> Result<ReviewResult, AppError> {
        let now = Utc::now().timestamp();
        
        let schedule = crate::db::repos::schedules::find_by_id(conn, schedule_id)?
            .ok_or_else(|| AppError::Other("Schedule not found".to_string()))?;

        let fsrs = FSRS::new(Some(&fsrs::DEFAULT_PARAMETERS))
            .map_err(|e| AppError::Other(format!("FSRS error: {}", e)))?;

        let days_elapsed = if let Some(lr) = schedule.last_reviewed {
            let diff = now - lr;
            (diff / 86400).max(0) as u32
        } else {
            0
        };

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

        let new_state = match rating {
            Rating::Again => scheduled_cards.again,
            Rating::Hard => scheduled_cards.hard,
            Rating::Good => scheduled_cards.good,
            Rating::Easy => scheduled_cards.easy,
        };

        let reps = schedule.reps + 1;
        let mut lapses = schedule.lapses;
        if rating == Rating::Again {
            lapses += 1;
        }

        let scheduled_days = new_state.interval;
        let next_review = now + (scheduled_days as f64 * 86400.0) as i64;
        let updated_state = if schedule.state == SchedulerState::New {
            SchedulerState::Learning
        } else {
            SchedulerState::Review
        };

        // 1. Log the review
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

        // 2. Update the schedule
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

    /// Schedules a note for review in `days` days. If no schedule exists for the note,
    /// a new one is created. Otherwise, the existing schedule's `next_review` is updated.
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

    pub fn get_hierarchical_due_counts(&self, conn: &Connection) -> Result<Vec<crate::domain::NoteDueCount>, AppError> {
        let now = Utc::now().timestamp();
        crate::db::repos::schedules::get_hierarchical_due_counts(conn, now)
    }

    pub fn get_repeat_mode_tree(&self, conn: &Connection) -> Result<Vec<crate::domain::RepeatModeNode>, AppError> {
        let now = Utc::now().timestamp();
        crate::db::repos::schedules::get_repeat_mode_tree(conn, now)
    }
}


