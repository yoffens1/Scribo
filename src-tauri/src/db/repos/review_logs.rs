use async_trait::async_trait;
use parking_lot::RwLock;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use std::sync::Arc;

use crate::domain::{ReviewLog, Rating, ScheduleId};
use super::{RepoError, ReviewLogsRepo};

pub struct SqliteReviewLogsRepo {
    pool: Arc<RwLock<Option<Pool<SqliteConnectionManager>>>>,
}

impl SqliteReviewLogsRepo {
    pub fn new(pool: Arc<RwLock<Option<Pool<SqliteConnectionManager>>>>) -> Self {
        Self { pool }
    }

    fn get_conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>, RepoError> {
        let pool_guard = self.pool.read();
        let pool = pool_guard.as_ref().ok_or_else(|| RepoError::Db("No DB pool".to_string()))?;
        pool.get().map_err(|e| RepoError::Db(e.to_string()))
    }
}

fn row_to_log(row: &rusqlite::Row) -> Result<ReviewLog, rusqlite::Error> {
    let rating_val: i32 = row.get(2)?;
    let rating = Rating::from_i32(rating_val).unwrap_or(Rating::Again);

    Ok(ReviewLog {
        log_id: row.get(0)?,
        schedule_id: ScheduleId(row.get(1)?),
        rating,
        reviewed_at: row.get(3)?,
        prev_stability: row.get(4)?,
        prev_difficulty: row.get(5)?,
        elapsed_days: row.get(6)?,
    })
}

#[async_trait]
impl ReviewLogsRepo for SqliteReviewLogsRepo {
    async fn insert(&self, log: &ReviewLog) -> Result<i64, RepoError> {
        let conn = self.get_conn()?;
        let id: i64 = conn.query_row(
            "INSERT INTO review_logs (schedule_id, rating, reviewed_at, prev_stability, prev_difficulty, elapsed_days)
             VALUES (?, ?, ?, ?, ?, ?) RETURNING log_id",
            rusqlite::params![
                log.schedule_id.0,
                log.rating.as_i32(),
                log.reviewed_at,
                log.prev_stability,
                log.prev_difficulty,
                log.elapsed_days
            ],
            |row| row.get(0)
        ).map_err(|e| RepoError::Db(e.to_string()))?;
        Ok(id)
    }

    async fn list_for_schedule(&self, id: ScheduleId) -> Result<Vec<ReviewLog>, RepoError> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT log_id, schedule_id, rating, reviewed_at, prev_stability, prev_difficulty, elapsed_days
             FROM review_logs WHERE schedule_id = ? ORDER BY reviewed_at ASC"
        ).map_err(|e| RepoError::Db(e.to_string()))?;

        let rows = stmt.query_map([id.0], row_to_log)
            .map_err(|e| RepoError::Db(e.to_string()))?;

        let mut res = Vec::new();
        for r in rows {
            res.push(r.map_err(|e| RepoError::Db(e.to_string()))?);
        }
        Ok(res)
    }
}
