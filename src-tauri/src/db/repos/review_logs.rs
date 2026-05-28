//! # Review Logs Repository
//!
//! Append-only log of every FSRS review event.
//! Rows are never updated or deleted — the table provides a full audit trail
//! that can be used to retrain FSRS parameters or export to Anki.
//!
//! `rating` is stored as an integer using FSRS's conventional encoding:
//! Again=1, Hard=2, Good=3, Easy=4.

use rusqlite::Connection;
use crate::error::AppError;
use crate::domain::{ReviewLog, Rating, ScheduleId};

/// Maps a row from `review_logs` to a [`ReviewLog`]. `rating` is decoded from its integer form.
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

/// Appends a new review log entry. Returns the auto-assigned `log_id`.
/// This function never updates or replaces existing rows.
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

/// Returns all review log entries for a schedule, ordered by `reviewed_at` ascending.
/// Useful for reconstructing the full review history and computing retention statistics.
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
