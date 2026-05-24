use tauri::State;
use crate::DbState;
use crate::error::AppError;
use crate::db::repos::files;
use crate::domain::file::{FileRef, UpsertIndexingParams, InsertFailedParams, FileQueryRecord};

#[tauri::command]
pub fn files_get_by_path(
    state: State<'_, DbState>,
    path: String,
) -> Result<Option<FileRef>, AppError> {
    state.with_conn(|conn| files::get_by_path(conn, &path))
}

#[tauri::command]
pub fn files_upsert_indexing(
    state: State<'_, DbState>,
    params: UpsertIndexingParams,
) -> Result<i64, AppError> {
    let _w = state.write_lock.lock();
    state.with_conn(|conn| files::upsert_indexing(conn, params))
}

#[tauri::command]
pub fn files_mark_indexed(app: tauri::AppHandle, state: State<'_, DbState>, file_id: i64) -> Result<(), AppError> {
    let res = state.with_conn(|conn| files::mark_indexed(conn, file_id));
    if res.is_ok() {
        use tauri::Emitter;
        let _ = app.emit("db:file:indexed", serde_json::json!({ "fileId": file_id }));
    }
    res
}

#[tauri::command]
pub fn files_mark_failed(
    state: State<'_, DbState>,
    path: String,
    error: String,
) -> Result<(), AppError> {
    state.with_conn(|conn| files::mark_failed(conn, &path, &error))
}

#[tauri::command]
pub fn files_insert_failed(
    state: State<'_, DbState>,
    params: InsertFailedParams,
) -> Result<(), AppError> {
    state.with_conn(|conn| files::insert_failed(conn, params))
}

#[tauri::command]
pub fn files_soft_delete(
    state: State<'_, DbState>,
    path: String,
    updated_at: i64,
) -> Result<(), AppError> {
    state.with_conn(|conn| files::soft_delete(conn, &path, updated_at))
}

#[tauri::command]
pub fn files_restore(
    state: State<'_, DbState>,
    path: String,
    updated_at: i64,
) -> Result<(), AppError> {
    state.with_conn(|conn| files::restore(conn, &path, updated_at))
}

#[tauri::command]
pub fn files_rename(
    state: State<'_, DbState>,
    old_path: String,
    new_path: String,
    updated_at: i64,
) -> Result<(), AppError> {
    state.with_conn(|conn| files::rename(conn, &old_path, &new_path, updated_at))
}

#[tauri::command]
pub fn files_count_chunks(state: State<'_, DbState>, path: String) -> Result<i64, AppError> {
    state.with_conn(|conn| files::count_chunks(conn, &path))
}

#[tauri::command]
pub fn files_hard_delete(state: State<'_, DbState>, path: String) -> Result<(), AppError> {
    state.with_conn(|conn| files::hard_delete(conn, &path))
}

#[tauri::command]
pub fn files_get_all(state: State<'_, DbState>) -> Result<Vec<FileQueryRecord>, AppError> {
    state.with_conn(|conn| files::get_all(conn))
}

#[tauri::command]
pub fn files_get_by_source_file_id(
    state: State<'_, DbState>,
    source_file_id: i64,
) -> Result<Vec<String>, AppError> {
    state.with_conn(|conn| files::get_by_source_file_id(conn, source_file_id))
}

#[tauri::command]
pub fn files_insert_minimal(
    state: State<'_, DbState>,
    path: String,
    name: String,
    hash: String,
) -> Result<i64, AppError> {
    state.with_conn(|conn| files::insert_minimal(conn, &path, &name, &hash))
}

#[tauri::command]
pub fn files_sync_upsert(
    state: State<'_, DbState>,
    path: String,
    name: String,
    hash: String,
    mtime: i64,
    source_file_id: Option<i64>,
) -> Result<i64, AppError> {
    let _w = state.write_lock.lock();
    state.with_conn(|conn| files::sync_upsert(conn, &path, &name, &hash, mtime, source_file_id))
}

#[tauri::command]
pub fn files_update_content_with_diff(
    state: State<'_, DbState>,
    file_id: i64,
    new_content: String,
) -> Result<(), AppError> {
    let _w = state.write_lock.lock();
    state.with_conn(|conn| files::update_content_with_diff(conn, file_id, &new_content))
}
