use rusqlite::{Connection, OptionalExtension};
use crate::error::AppError;
use crate::domain::{NewSchedule, ReviewTarget, ReviewTargetType, Schedule, ScheduleId, SchedulerState, Timestamp};

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

pub fn find_by_id(conn: &Connection, id: ScheduleId) -> Result<Option<Schedule>, AppError> {
    let res = conn.query_row(
        "SELECT schedule_id, target_type, target_id, state, stability, difficulty, reps, lapses, last_reviewed, next_review 
         FROM schedules WHERE schedule_id = ?",
        [id.0],
        row_to_schedule,
    ).optional()?;
    Ok(res)
}

pub fn find_by_target(conn: &Connection, target: ReviewTarget) -> Result<Option<Schedule>, AppError> {
    let res = conn.query_row(
        "SELECT schedule_id, target_type, target_id, state, stability, difficulty, reps, lapses, last_reviewed, next_review 
         FROM schedules WHERE target_type = ? AND target_id = ?",
        rusqlite::params![target.target_type().as_str(), target.target_id()],
        row_to_schedule,
    ).optional()?;
    Ok(res)
}

pub fn find_due(conn: &Connection, now: Timestamp, limit: i64) -> Result<Vec<Schedule>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT schedule_id, target_type, target_id, state, stability, difficulty, reps, lapses, last_reviewed, next_review 
         FROM schedules 
         WHERE next_review IS NOT NULL AND next_review <= ? 
         ORDER BY next_review ASC LIMIT ?"
    )?;

    let rows = stmt.query_map(rusqlite::params![now, limit], row_to_schedule)?;
    let mut res = Vec::new();
    for r in rows {
        res.push(r?);
    }
    Ok(res)
}

pub fn count_due(conn: &Connection, now: Timestamp) -> Result<i64, AppError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM schedules WHERE next_review IS NOT NULL AND next_review <= ?",
        [now],
        |r| r.get(0)
    )?;
    Ok(count)
}

pub fn insert(conn: &Connection, new: NewSchedule) -> Result<ScheduleId, AppError> {
    let id: i64 = conn.query_row(
        "INSERT INTO schedules (target_type, target_id, state, next_review) 
         VALUES (?, ?, ?, ?) RETURNING schedule_id",
        rusqlite::params![
            new.target.target_type().as_str(), 
            new.target.target_id(),
            SchedulerState::New.as_str(),
            new.initial_due
        ],
        |row| row.get(0)
    )?;
    Ok(ScheduleId(id))
}

pub fn update_state(
    conn: &Connection,
    id: ScheduleId,
    state: SchedulerState,
    stability: f64,
    difficulty: f64,
    reps: i64,
    lapses: i64,
    last_reviewed: Option<Timestamp>,
    next_review: Option<Timestamp>,
) -> Result<(), AppError> {
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
    )?;
    Ok(())
}

pub fn set_next_review(
    conn: &Connection,
    id: ScheduleId,
    next_review: Option<Timestamp>,
) -> Result<(), AppError> {
    conn.execute(
        "UPDATE schedules SET next_review = ? WHERE schedule_id = ?",
        rusqlite::params![next_review, id.0]
    )?;
    Ok(())
}

pub fn delete_by_target(conn: &Connection, target: ReviewTarget) -> Result<(), AppError> {
    conn.execute(
        "DELETE FROM schedules WHERE target_type = ? AND target_id = ?",
        rusqlite::params![target.target_type().as_str(), target.target_id()]
    )?;
    Ok(())
}
