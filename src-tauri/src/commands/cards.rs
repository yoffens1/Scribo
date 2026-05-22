use crate::error::AppError;
use crate::DbState;
use tauri::State;
use serde::{Serialize, Deserialize};
use chrono::Utc;
use fsrs::{FSRS, MemoryState};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardReviewParams {
    pub card_id: i64,
    /// Оценка пользователя: 1 (Again), 2 (Hard), 3 (Good), 4 (Easy)
    pub rating: u32, 
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewResult {
    pub scheduled_days: f32,
    pub next_review: i64,
}

#[tauri::command]
pub fn cards_insert_ignore(state: State<'_, DbState>, file_id: i64) -> Result<(), AppError> {
    state.with_conn(|conn| {
        conn.execute(
            "INSERT OR IGNORE INTO cards (file_id, state, reps, lapses, stability, difficulty)
             VALUES (?, 'new', 0, 0, 0.0, 0.0)",
            rusqlite::params![file_id],
        )?;
        Ok(())
    })
}

#[tauri::command]
pub fn cards_review_fsrs_impl(
    state: &DbState,
    params: CardReviewParams,
) -> Result<ReviewResult, AppError> {
    let _w = state.write_lock.lock();
    let now = Utc::now();

    state.with_conn(|conn| {
        let tx = conn.transaction()?;

        // 1. Load current parameters. Since we only have interval_days (lapses) and ease_factor (stability)
        // we'll use a fixed difficulty for the missing state, or re-compute.
        let (mut reps, mut lapses, stability, difficulty, last_reviewed, state_str) = tx.query_row(
            "SELECT reps, lapses, stability, difficulty, last_reviewed, state FROM cards WHERE card_id = ?",
            rusqlite::params![params.card_id],
            |row| {
                Ok((
                    row.get::<_, i32>(0)?,
                    row.get::<_, i32>(1)?, // lapses
                    row.get::<_, f64>(2)?, // stability
                    row.get::<_, f64>(3)?, // difficulty
                    row.get::<_, Option<i64>>(4)?, // last_reviewed timestamp
                    row.get::<_, String>(5)?, // state
                ))
            },
        )?;

        // FSRS v5 inference API requires parameters to be set on creation. We use the default ones.
        let fsrs = FSRS::new(Some(&fsrs::DEFAULT_PARAMETERS)).map_err(|e| AppError::Other(e.to_string()))?;
        
        // Compute elapsed days
        let days_elapsed = if let Some(lr) = last_reviewed {
            let diff = now.timestamp() - lr;
            (diff / 86400).max(0) as u32
        } else {
            0
        };

        // Reconstruct MemoryState
        let current_memory_state = if state_str == "new" {
            None
        } else {
            Some(MemoryState {
                stability: stability as f32,
                difficulty: difficulty as f32,
            })
        };

        // Get next states from FSRS scheduler (default desired retention = 0.90)
        let scheduled_cards = fsrs.next_states(current_memory_state, 0.90, days_elapsed).map_err(|e| AppError::Other(e.to_string()))?;
        
        let new_state = match params.rating {
            1 => scheduled_cards.again,
            2 => scheduled_cards.hard,
            3 => scheduled_cards.good,
            _ => scheduled_cards.easy,
        };

        // Update reps/lapses manually since FSRS v5 ItemState only tracks memory and interval
        reps += 1;
        if params.rating == 1 {
            lapses += 1;
        }

        let scheduled_days = new_state.interval;
        let next_review = now.timestamp() + (scheduled_days as f64 * 86400.0) as i64;
        let updated_state_str = if state_str == "new" { "learning" } else { "review" };

        // 5. Save back to SQLite
        tx.execute(
            "UPDATE cards SET 
                state = ?, 
                reps = ?, 
                lapses = ?,
                stability = ?,
                difficulty = ?,
                next_review = ?, 
                last_reviewed = ? 
             WHERE card_id = ?",
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

        // 6. Log review
        tx.execute(
            "INSERT INTO review_logs (card_id, rating, reviewed_at) VALUES (?, ?, ?)",
            rusqlite::params![params.card_id, params.rating, now.timestamp()],
        )?;

        tx.commit()?;

        Ok(ReviewResult {
            scheduled_days,
            next_review,
        })
    })
}

#[tauri::command]
pub fn cards_review_fsrs(
    state: State<'_, DbState>,
    params: CardReviewParams,
) -> Result<ReviewResult, AppError> {
    cards_review_fsrs_impl(&state, params)
}
