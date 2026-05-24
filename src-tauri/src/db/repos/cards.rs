use rusqlite::Connection;
use chrono::Utc;
use fsrs::{FSRS, MemoryState};
use crate::error::AppError;
use crate::domain::card::{CardReviewParams, ReviewResult};





pub fn insert_ignore(conn: &Connection, note_id: i64) -> Result<(), AppError> {
    conn.execute(
        "INSERT OR IGNORE INTO cards (note_id, state, reps, lapses, stability, difficulty)
         VALUES (?, 'new', 0, 0, 0.0, 0.0)",
        rusqlite::params![note_id],
    )?;
    Ok(())
}

pub fn review_fsrs(
    conn: &mut Connection,
    params: CardReviewParams,
) -> Result<ReviewResult, AppError> {
    let now = Utc::now();
    let tx = conn.transaction()?;

    let (mut reps, mut lapses, stability, difficulty, last_reviewed, state_str) = tx.query_row(
        "SELECT reps, lapses, stability, difficulty, last_reviewed, state FROM schedules WHERE target_type = 'card' AND target_id = ?",
        rusqlite::params![params.card_id],
        |row| {
            Ok((
                row.get::<_, i32>(0)?,
                row.get::<_, i32>(1)?,
                row.get::<_, f64>(2)?,
                row.get::<_, f64>(3)?,
                row.get::<_, Option<i64>>(4)?,
                row.get::<_, String>(5)?,
            ))
        },
    )?;

    let fsrs = FSRS::new(Some(&fsrs::DEFAULT_PARAMETERS)).map_err(|e| AppError::Other(e.to_string()))?;
    
    let days_elapsed = if let Some(lr) = last_reviewed {
        let diff = now.timestamp() - lr;
        (diff / 86400).max(0) as u32
    } else {
        0
    };

    let current_memory_state = if state_str == "new" {
        None
    } else {
        Some(MemoryState {
            stability: stability as f32,
            difficulty: difficulty as f32,
        })
    };

    let scheduled_cards = fsrs.next_states(current_memory_state, 0.90, days_elapsed).map_err(|e| AppError::Other(e.to_string()))?;
    
    let new_state = match params.rating {
        1 => scheduled_cards.again,
        2 => scheduled_cards.hard,
        3 => scheduled_cards.good,
        _ => scheduled_cards.easy,
    };

    reps += 1;
    if params.rating == 1 {
        lapses += 1;
    }

    let scheduled_days = new_state.interval;
    let next_review = now.timestamp() + (scheduled_days as f64 * 86400.0) as i64;
    let updated_state_str = if state_str == "new" { "learning" } else { "review" };

    tx.execute(
        "UPDATE schedules SET 
            state = ?, 
            reps = ?, 
            lapses = ?,
            stability = ?,
            difficulty = ?,
            next_review = ?, 
            last_reviewed = ? 
         WHERE target_type = 'card' AND target_id = ?",
        rusqlite::params![
            updated_state_str,
            reps,
            lapses,
            new_state.memory.stability as f64,
            new_state.memory.difficulty as f64,
            next_review,
            now.timestamp(),
            params.card_id
        ],
    )?;

    tx.execute(
        "INSERT INTO review_logs (card_id, rating, reviewed_at) VALUES (?, ?, ?)",
        rusqlite::params![params.card_id, params.rating, now.timestamp()],
    )?;

    tx.commit()?;

    Ok(ReviewResult {
        success: true,
        scheduled_days: scheduled_days as i64,
        next_review,
    })
}
