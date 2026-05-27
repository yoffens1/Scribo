use rusqlite::Connection;
use crate::error::AppError;
use crate::domain::{ReviewLog, Rating, ScheduleId};

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

pub fn insert(conn: &Connection, log: &ReviewLog) -> Result<i64, AppError> {
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
    )?;
    Ok(id)
}

pub fn list_for_schedule(conn: &Connection, id: ScheduleId) -> Result<Vec<ReviewLog>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT log_id, schedule_id, rating, reviewed_at, prev_stability, prev_difficulty, elapsed_days
         FROM review_logs WHERE schedule_id = ? ORDER BY reviewed_at ASC"
    )?;

    let rows = stmt.query_map([id.0], row_to_log)?;
    let mut res = Vec::new();
    for r in rows {
        res.push(r?);
    }
    Ok(res)
}
