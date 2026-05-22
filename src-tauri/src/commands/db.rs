use tauri::State;
use crate::DbState;
use crate::error::AppError;

#[tauri::command]
pub fn db_begin_transaction(state: State<'_, DbState>) -> Result<(), AppError> {
    let mut opt_conn = state.0.lock();
    let conn = opt_conn.as_mut().ok_or(AppError::NotInitialized)?;
    conn.execute_batch("BEGIN TRANSACTION;")?;
    Ok(())
}

#[tauri::command]
pub fn db_commit_transaction(state: State<'_, DbState>) -> Result<(), AppError> {
    let mut opt_conn = state.0.lock();
    let conn = opt_conn.as_mut().ok_or(AppError::NotInitialized)?;
    conn.execute_batch("COMMIT;")?;
    Ok(())
}

#[tauri::command]
pub fn db_rollback_transaction(state: State<'_, DbState>) -> Result<(), AppError> {
    let mut opt_conn = state.0.lock();
    let conn = opt_conn.as_mut().ok_or(AppError::NotInitialized)?;
    let _ = conn.execute_batch("ROLLBACK;");
    Ok(())
}

#[tauri::command]
pub fn db_vacuum(state: State<'_, DbState>) -> Result<(), AppError> {
    let mut opt_conn = state.0.lock();
    let conn = opt_conn.as_mut().ok_or(AppError::NotInitialized)?;
    conn.execute_batch("VACUUM;")?;
    Ok(())
}

#[tauri::command]
pub fn db_optimize(state: State<'_, DbState>) -> Result<(), AppError> {
    let mut opt_conn = state.0.lock();
    let conn = opt_conn.as_mut().ok_or(AppError::NotInitialized)?;
    conn.execute_batch("PRAGMA optimize;")?;
    Ok(())
}
