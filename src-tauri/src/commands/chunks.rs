use tauri::State;
use crate::DbState;
use crate::error::AppError;
use crate::db::repos::chunks;
use crate::domain::chunk::{ChunkInsertRow, Chunk, SearchHit, VectorSearchHit};

#[tauri::command]
pub fn chunks_delete_by_file_id(
    state: State<'_, DbState>,
    file_id: i64,
) -> Result<i64, AppError> {
    let _w = state.write_lock.lock();
    state.with_conn(|conn| chunks::delete_by_file_id(conn, file_id))
}

#[tauri::command]
pub fn chunks_insert(
    app: tauri::AppHandle,
    state: State<'_, DbState>,
    file_id: i64,
    rows: Vec<ChunkInsertRow>,
) -> Result<(), AppError> {
    let _w = state.write_lock.lock();
    let rows_len = rows.len();
    let res = state.with_conn(|conn| chunks::insert(conn, file_id, rows));
    if res.is_ok() {
        use tauri::Emitter;
        let _ = app.emit("db:chunk:inserted", serde_json::json!({ "fileId": file_id, "count": rows_len }));
    }
    res
}

#[tauri::command]
pub fn chunks_get_by_file_path(
    state: State<'_, DbState>,
    file_path: String,
    include_deleted: bool,
) -> Result<Vec<Chunk>, AppError> {
    state.with_conn(|conn| chunks::get_by_file_path(conn, &file_path, include_deleted))
}

#[tauri::command]
pub fn chunks_get_all(
    state: State<'_, DbState>,
    include_deleted: bool,
) -> Result<Vec<Chunk>, AppError> {
    state.with_conn(|conn| chunks::get_all(conn, include_deleted))
}

#[tauri::command]
pub fn chunks_get_by_file_name(
    state: State<'_, DbState>,
    name: String,
    include_deleted: bool,
) -> Result<Vec<Chunk>, AppError> {
    state.with_conn(|conn| chunks::get_by_file_name(conn, &name, include_deleted))
}

#[tauri::command]
pub fn chunks_search(
    state: State<'_, DbState>,
    query: String,
    limit: i64,
) -> Result<Vec<SearchHit>, AppError> {
    state.with_conn(|conn| chunks::search(conn, &query, limit))
}

#[tauri::command]
pub async fn chunks_vector_search(
    state: State<'_, DbState>,
    query_embedding_bytes: Vec<u8>,
    limit: usize,
) -> Result<Vec<VectorSearchHit>, AppError> {
    let pool = state.inner().pool.read().as_ref().cloned().ok_or(AppError::NotInitialized)?;

    tauri::async_runtime::spawn_blocking(move || {
        let conn = pool.get().map_err(|e| AppError::Other(e.to_string()))?;
        chunks::vector_search(&conn, &query_embedding_bytes, limit)
    })
    .await
    .map_err(|e| AppError::Other(e.to_string()))?
}
