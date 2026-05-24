use async_trait::async_trait;
use rusqlite::OptionalExtension;
use parking_lot::RwLock;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use std::sync::Arc;

use crate::domain::{NewSchedule, ReviewTarget, ReviewTargetType, Schedule, ScheduleId, SchedulerState, Timestamp};
use super::{RepoError, SchedulesRepo};

pub struct SqliteSchedulesRepo {
    pool: Arc<RwLock<Option<Pool<SqliteConnectionManager>>>>,
}

impl SqliteSchedulesRepo {
    pub fn new(pool: Arc<RwLock<Option<Pool<SqliteConnectionManager>>>>) -> Self {
        Self { pool }
    }

    fn get_conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>, RepoError> {
        let pool_guard = self.pool.read();
        let pool = pool_guard.as_ref().ok_or_else(|| RepoError::Db("No DB pool".to_string()))?;
        pool.get().map_err(|e| RepoError::Db(e.to_string()))
    }
}

fn row_to_schedule(row: &rusqlite::Row) -> Result<Schedule, rusqlite::Error> {
    let id: i64 = row.get(0)?;
    let target_type_str: String = row.get(1)?;
    let target_id: i64 = row.get(2)?;
    let state_str: String = row.get(3)?;
    
    let target_type = ReviewTargetType::parse(&target_type_str)
        .unwrap_or(ReviewTargetType::Card);
    let target = ReviewTarget::from_parts(target_type, target_id);
    
    let state = SchedulerState::parse(&state_str).unwrap_or(SchedulerState::New);

    Ok(Schedule {
        id: ScheduleId(id),
        target,
        state,
        stability: row.get(4)?,
        difficulty: row.get(5)?,
        reps: row.get(6)?,
        lapses: row.get(7)?,
        last_reviewed: row.get::<_, Option<i64>>(8)?,
        next_review: row.get::<_, Option<i64>>(9)?,
    })
}

#[async_trait]
impl SchedulesRepo for SqliteSchedulesRepo {
    async fn find_by_id(&self, id: ScheduleId) -> Result<Option<Schedule>, RepoError> {
        let conn = self.get_conn()?;
        let res = conn.query_row(
            "SELECT schedule_id, target_type, target_id, state, stability, difficulty, reps, lapses, last_reviewed, next_review 
             FROM schedules WHERE schedule_id = ?",
            [id.0],
            row_to_schedule,
        ).optional()
        .map_err(|e| RepoError::Db(e.to_string()))?;
        Ok(res)
    }

    async fn find_by_target(&self, target: ReviewTarget) -> Result<Option<Schedule>, RepoError> {
        let conn = self.get_conn()?;
        let res = conn.query_row(
            "SELECT schedule_id, target_type, target_id, state, stability, difficulty, reps, lapses, last_reviewed, next_review 
             FROM schedules WHERE target_type = ? AND target_id = ?",
            rusqlite::params![target.target_type().as_str(), target.target_id()],
            row_to_schedule,
        ).optional()
        .map_err(|e| RepoError::Db(e.to_string()))?;
        Ok(res)
    }

    async fn find_due(&self, now: Timestamp, limit: i64) -> Result<Vec<Schedule>, RepoError> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT schedule_id, target_type, target_id, state, stability, difficulty, reps, lapses, last_reviewed, next_review 
             FROM schedules 
             WHERE next_review IS NOT NULL AND next_review <= ? 
             ORDER BY next_review ASC LIMIT ?"
        ).map_err(|e| RepoError::Db(e.to_string()))?;

        let rows = stmt.query_map(rusqlite::params![now, limit], row_to_schedule)
            .map_err(|e| RepoError::Db(e.to_string()))?;
        
        let mut res = Vec::new();
        for r in rows {
            res.push(r.map_err(|e| RepoError::Db(e.to_string()))?);
        }
        Ok(res)
    }

    async fn count_due(&self, now: Timestamp) -> Result<i64, RepoError> {
        let conn = self.get_conn()?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM schedules WHERE next_review IS NOT NULL AND next_review <= ?",
            [now],
            |r| r.get(0)
        ).map_err(|e| RepoError::Db(e.to_string()))?;
        Ok(count)
    }

    async fn insert(&self, new: NewSchedule) -> Result<ScheduleId, RepoError> {
        let conn = self.get_conn()?;
        let id: i64 = conn.query_row(
            "INSERT INTO schedules (target_type, target_id, state, next_review) 
             VALUES (?, ?, 'new', ?) RETURNING schedule_id",
            rusqlite::params![
                new.target.target_type().as_str(), 
                new.target.target_id(),
                new.initial_due
            ],
            |row| row.get(0)
        ).map_err(|e| RepoError::Db(e.to_string()))?;
        Ok(ScheduleId(id))
    }

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
    ) -> Result<(), RepoError> {
        let conn = self.get_conn()?;
        conn.execute(
            "UPDATE schedules SET state = ?, stability = ?, difficulty = ?, reps = ?, lapses = ?, last_reviewed = ?, next_review = ? WHERE schedule_id = ?",
            rusqlite::params![
                state.as_str(),
                stability,
                difficulty,
                reps,
                lapses,
                last_reviewed,
                next_review,
                id.0
            ]
        ).map_err(|e| RepoError::Db(e.to_string()))?;
        Ok(())
    }

    async fn set_next_review(
        &self,
        id: ScheduleId,
        next_review: Option<Timestamp>,
    ) -> Result<(), RepoError> {
        let conn = self.get_conn()?;
        conn.execute(
            "UPDATE schedules SET next_review = ? WHERE schedule_id = ?",
            rusqlite::params![next_review, id.0]
        ).map_err(|e| RepoError::Db(e.to_string()))?;
        Ok(())
    }

    async fn delete_by_target(&self, target: ReviewTarget) -> Result<(), RepoError> {
        let conn = self.get_conn()?;
        conn.execute(
            "DELETE FROM schedules WHERE target_type = ? AND target_id = ?",
            rusqlite::params![target.target_type().as_str(), target.target_id()]
        ).map_err(|e| RepoError::Db(e.to_string()))?;
        Ok(())
    }
}
