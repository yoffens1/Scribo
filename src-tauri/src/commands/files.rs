use rusqlite::OptionalExtension;
use tauri::State;
use serde::{Deserialize, Serialize};
use crate::DbState;
use crate::error::AppError;

#[derive(Serialize, Deserialize)]
pub struct FileRecord {
    pub file_id: i64,
    pub file_hash: Option<String>,
    pub is_deleted: Option<i64>,
    pub model: Option<String>,
    pub chunk_version: Option<String>,
    pub mtime: Option<i64>,
}

#[tauri::command]
pub fn files_get_by_path(
    state: State<'_, DbState>,
    path: String,
) -> Result<Option<FileRecord>, AppError> {
    let mut opt_conn = state.0.lock();
    let conn = opt_conn.as_mut().ok_or(AppError::NotInitialized)?;

    let mut stmt = conn.prepare(
        "SELECT file_id, file_hash, is_deleted, embedding_model, chunking_version, file_mtime 
         FROM files WHERE file_path = ?"
    )?;

    let mut rows = stmt.query([path])?;
    if let Some(row) = rows.next()? {
        Ok(Some(FileRecord {
            file_id: row.get(0)?,
            file_hash: row.get(1)?,
            is_deleted: row.get(2)?,
            model: row.get(3)?,
            chunk_version: row.get(4)?,
            mtime: row.get(5)?,
        }))
    } else {
        Ok(None)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsertIndexingParams {
    pub clean_path: String,
    pub file_name: String,
    pub file_hash: String,
    pub file_mtime: Option<i64>,
    pub embedding_model: String,
    pub embedding_dim: i64,
    pub chunking_version: String,
    pub updated_at: i64,
}

#[tauri::command]
pub fn files_insert_indexing(
    state: State<'_, DbState>,
    params: InsertIndexingParams,
) -> Result<i64, AppError> {
    let mut opt_conn = state.0.lock();
    let conn = opt_conn.as_mut().ok_or(AppError::NotInitialized)?;

    conn.execute(
        "INSERT INTO files (
            file_path, file_name, file_hash, file_mtime, 
            embedding_model, embedding_dimension, chunking_version, 
            status, updated_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, 'indexing', ?)",
        rusqlite::params![
            params.clean_path,
            params.file_name,
            params.file_hash,
            params.file_mtime,
            params.embedding_model,
            params.embedding_dim,
            params.chunking_version,
            params.updated_at
        ],
    )?;

    Ok(conn.last_insert_rowid())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateIndexingParams {
    pub file_hash: String,
    pub file_mtime: Option<i64>,
    pub embedding_model: String,
    pub embedding_dim: i64,
    pub chunking_version: String,
    pub updated_at: i64,
    pub file_name: String,
    pub file_id: i64,
}

#[tauri::command]
pub fn files_update_indexing(
    state: State<'_, DbState>,
    params: UpdateIndexingParams,
) -> Result<(), AppError> {
    let mut opt_conn = state.0.lock();
    let conn = opt_conn.as_mut().ok_or(AppError::NotInitialized)?;

    conn.execute(
        "UPDATE files SET 
            file_hash = ?, file_mtime = ?, embedding_model = ?, 
            embedding_dimension = ?, chunking_version = ?, 
            updated_at = ?, file_name = ?, is_deleted = 0, 
            status = 'indexing' 
         WHERE file_id = ?",
        rusqlite::params![
            params.file_hash,
            params.file_mtime,
            params.embedding_model,
            params.embedding_dim,
            params.chunking_version,
            params.updated_at,
            params.file_name,
            params.file_id
        ],
    )?;

    Ok(())
}

#[tauri::command]
pub fn files_exists(state: State<'_, DbState>, path: String) -> Result<bool, AppError> {
    let mut opt_conn = state.0.lock();
    let conn = opt_conn.as_mut().ok_or(AppError::NotInitialized)?;

    let mut stmt = conn.prepare("SELECT 1 FROM files WHERE file_path = ?")?;
    let mut rows = stmt.query([path])?;
    Ok(rows.next()?.is_some())
}

#[tauri::command]
pub fn files_mark_indexed(state: State<'_, DbState>, file_id: i64) -> Result<(), AppError> {
    let mut opt_conn = state.0.lock();
    let conn = opt_conn.as_mut().ok_or(AppError::NotInitialized)?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
    conn.execute(
        "UPDATE files SET status = 'indexed', last_error = NULL, indexed_at = ? WHERE file_id = ?",
        rusqlite::params![now, file_id],
    )?;
    Ok(())
}

#[tauri::command]
pub fn files_mark_failed(state: State<'_, DbState>, path: String, error: String) -> Result<(), AppError> {
    let mut opt_conn = state.0.lock();
    let conn = opt_conn.as_mut().ok_or(AppError::NotInitialized)?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
    // Log failure. We ignore error per TS implementation.
    let _ = conn.execute(
        "INSERT INTO files (file_path, file_name, status, last_error, updated_at) 
         VALUES (?, ?, 'failed', ?, ?) 
         ON CONFLICT(file_path) DO UPDATE SET status = 'failed', last_error = excluded.last_error",
        rusqlite::params![path.clone(), path, error, now],
    );
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsertFailedParams {
    pub clean_path: String,
    pub file_name: String,
    pub file_hash: String,
    pub file_mtime: Option<i64>,
    pub error: String,
    pub updated_at: i64,
}

#[tauri::command]
pub fn files_insert_failed(state: State<'_, DbState>, params: InsertFailedParams) -> Result<(), AppError> {
    let mut opt_conn = state.0.lock();
    let conn = opt_conn.as_mut().ok_or(AppError::NotInitialized)?;
    conn.execute(
        "INSERT INTO files (file_path, file_name, file_hash, file_mtime, embedding_model, status, last_error, updated_at) 
         VALUES (?, ?, ?, ?, 'unknown', 'failed', ?, ?) 
         ON CONFLICT(file_path) DO UPDATE SET 
            file_name = excluded.file_name, file_hash = excluded.file_hash, 
            file_mtime = excluded.file_mtime, embedding_model = excluded.embedding_model, 
            status = excluded.status, last_error = excluded.last_error, updated_at = excluded.updated_at",
        rusqlite::params![
            params.clean_path, params.file_name, params.file_hash, 
            params.file_mtime, params.error, params.updated_at
        ],
    )?;
    Ok(())
}

#[tauri::command]
pub fn files_soft_delete(state: State<'_, DbState>, path: String, updated_at: i64) -> Result<(), AppError> {
    let mut opt_conn = state.0.lock();
    let conn = opt_conn.as_mut().ok_or(AppError::NotInitialized)?;
    conn.execute("UPDATE files SET is_deleted = 1, updated_at = ? WHERE file_path = ?", rusqlite::params![updated_at, path])?;
    Ok(())
}

#[tauri::command]
pub fn files_restore(state: State<'_, DbState>, path: String, updated_at: i64) -> Result<(), AppError> {
    let mut opt_conn = state.0.lock();
    let conn = opt_conn.as_mut().ok_or(AppError::NotInitialized)?;
    conn.execute("UPDATE files SET is_deleted = 0, updated_at = ? WHERE file_path = ?", rusqlite::params![updated_at, path])?;
    Ok(())
}

#[tauri::command]
pub fn files_rename(state: State<'_, DbState>, old_path: String, new_path: String, updated_at: i64) -> Result<(), AppError> {
    let mut opt_conn = state.0.lock();
    let conn = opt_conn.as_mut().ok_or(AppError::NotInitialized)?;
    let new_name = new_path.split('/').last().unwrap_or(&new_path).to_string();
    conn.execute(
        "UPDATE files SET file_path = ?, file_name = ?, updated_at = ? WHERE file_path = ?", 
        rusqlite::params![new_path, new_name, updated_at, old_path]
    )?;
    Ok(())
}

#[tauri::command]
pub fn files_count_chunks(state: State<'_, DbState>, path: String) -> Result<i64, AppError> {
    let mut opt_conn = state.0.lock();
    let conn = opt_conn.as_mut().ok_or(AppError::NotInitialized)?;
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM chunks WHERE file_id IN (SELECT file_id FROM files WHERE file_path = ?)",
        rusqlite::params![path],
        |row| row.get(0)
    )?;
    Ok(count)
}

#[tauri::command]
pub fn files_hard_delete(state: State<'_, DbState>, path: String) -> Result<(), AppError> {
    let mut opt_conn = state.0.lock();
    let conn = opt_conn.as_mut().ok_or(AppError::NotInitialized)?;
    conn.execute("DELETE FROM files WHERE file_path = ?", rusqlite::params![path])?;
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileQueryRecord {
    pub file_id: i64,
    pub file_path: String,
    pub is_deleted: i64,
    pub mtime: Option<i64>,
    pub model: Option<String>,
    pub chunk_ver: Option<String>,
}

#[tauri::command]
pub fn files_get_all(state: State<'_, DbState>) -> Result<Vec<FileQueryRecord>, AppError> {
    let mut opt_conn = state.0.lock();
    let conn = opt_conn.as_mut().ok_or(AppError::NotInitialized)?;
    let mut stmt = conn.prepare("SELECT file_id, file_path, is_deleted, file_mtime, embedding_model, chunking_version FROM files")?;
    let rows = stmt.query_map([], |row| {
        Ok(FileQueryRecord {
            file_id: row.get(0)?,
            file_path: row.get(1)?,
            is_deleted: row.get(2)?,
            mtime: row.get(3)?,
            model: row.get(4)?,
            chunk_ver: row.get(5)?,
        })
    })?;
    
    let mut res = Vec::new();
    for r in rows { res.push(r?); }
    Ok(res)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileMapRecord {
    pub is_deleted: bool,
    pub mtime: Option<i64>,
    pub model: Option<String>,
    pub chunk_ver: Option<String>,
}

#[tauri::command]
pub fn files_get_map(state: State<'_, DbState>) -> Result<std::collections::HashMap<String, FileMapRecord>, AppError> {
    let mut opt_conn = state.0.lock();
    let conn = opt_conn.as_mut().ok_or(AppError::NotInitialized)?;
    let mut stmt = conn.prepare("SELECT file_path, is_deleted, file_mtime, embedding_model, chunking_version FROM files")?;
    let rows = stmt.query_map([], |row| {
        let is_deleted: i64 = row.get(1)?;
        Ok((row.get::<_, String>(0)?, FileMapRecord {
            is_deleted: is_deleted == 1,
            mtime: row.get(2)?,
            model: row.get(3)?,
            chunk_ver: row.get(4)?,
        }))
    })?;
    
    let mut res = std::collections::HashMap::new();
    for r in rows {
        let (k, v) = r?;
        res.insert(k, v);
    }
    Ok(res)
}

#[tauri::command]
pub fn files_get_by_source_file_id(state: State<'_, DbState>, source_file_id: i64) -> Result<Vec<String>, AppError> {
    let mut opt_conn = state.0.lock();
    let conn = opt_conn.as_mut().ok_or(AppError::NotInitialized)?;
    let mut stmt = conn.prepare("SELECT file_path FROM files WHERE source_file_id = ? AND is_deleted = 0")?;
    let rows = stmt.query_map(rusqlite::params![source_file_id], |row| row.get(0))?;
    
    let mut res = Vec::new();
    for r in rows { res.push(r?); }
    Ok(res)
}

#[tauri::command]
pub fn files_insert_minimal(state: State<'_, DbState>, path: String, name: String, hash: String) -> Result<i64, AppError> {
    let mut opt_conn = state.0.lock();
    let conn = opt_conn.as_mut().ok_or(AppError::NotInitialized)?;
    conn.execute(
        "INSERT INTO files (file_path, file_name, file_hash, status) VALUES (?, ?, ?, 'indexed')",
        rusqlite::params![path, name, hash],
    )?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn files_sync_upsert(
    state: State<'_, DbState>, 
    path: String, 
    name: String, 
    hash: String, 
    mtime: i64, 
    source_file_id: Option<i64>
) -> Result<i64, AppError> {
    let mut opt_conn = state.0.lock();
    let conn = opt_conn.as_mut().ok_or(AppError::NotInitialized)?;

    let mut stmt = conn.prepare("SELECT file_id FROM files WHERE file_path = ?")?;
    let file_id: Option<i64> = stmt.query_row(rusqlite::params![path], |row| row.get(0)).optional()?;

    let updated_at = mtime;

    if let Some(id) = file_id {
        conn.execute(
            "UPDATE files SET file_hash = ?, file_mtime = ?, source_file_id = ?, is_deleted = 0, status = 'indexed', updated_at = ? WHERE file_id = ?",
            rusqlite::params![hash, mtime, source_file_id, updated_at, id],
        )?;
        Ok(id)
    } else {
        conn.execute(
            "INSERT INTO files (file_path, file_name, file_hash, file_mtime, source_file_id, is_deleted, status, updated_at)
             VALUES (?, ?, ?, ?, ?, 0, 'indexed', ?)",
            rusqlite::params![path, name, hash, mtime, source_file_id, updated_at],
        )?;
        Ok(conn.last_insert_rowid())
    }
}

