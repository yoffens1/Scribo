use tauri::State;
use crate::DbState;
use crate::error::AppError;

#[tauri::command]
pub fn cards_insert_ignore(state: State<'_, DbState>, file_id: i64) -> Result<(), AppError> {
    let mut opt_conn = state.0.lock();
    let conn = opt_conn.as_mut().ok_or(AppError::NotInitialized)?;
    conn.execute(
        "INSERT OR IGNORE INTO cards (file_id, state, reps, interval_days, ease_factor)
         VALUES (?, 'new', 0, 0, 2.5)",
        rusqlite::params![file_id],
    )?;
    Ok(())
}
