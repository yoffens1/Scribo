use crate::error::AppError;
use crate::DbState;
use tauri::State;

/// VACUUM is the most expensive SQLite operation (~O(db_size)).
/// We run it in a dedicated blocking thread to avoid tying up Tauri's
/// async IPC worker pool for potentially several seconds.
#[tauri::command]
pub async fn db_vacuum(state: State<'_, DbState>) -> Result<(), AppError> {
    let pool = state.inner().pool.read().as_ref().cloned().ok_or(AppError::NotInitialized)?;

    tauri::async_runtime::spawn_blocking(move || {
        let conn = pool.get().map_err(|e| AppError::Other(e.to_string()))?;
        conn.execute_batch("VACUUM;")?;
        Ok::<(), AppError>(())
    })
    .await
    .map_err(|e| AppError::Other(e.to_string()))??;

    Ok(())
}

#[tauri::command]
pub fn db_optimize(state: State<'_, DbState>) -> Result<(), AppError> {
    state.with_conn(|conn| {
        conn.execute_batch("PRAGMA optimize;")?;
        Ok(())
    })
}
