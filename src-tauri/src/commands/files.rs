use rusqlite::OptionalExtension;
use tauri::State;
use serde::{Deserialize, Serialize};
use crate::DbState;
use crate::error::AppError;

// ── Shared helper ─────────────────────────────────────────────────────────────

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

// ── files_get_by_path ─────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileRecord {
    pub file_id: i64,
    pub file_hash: Option<String>,
    pub is_deleted: Option<i64>,
    pub embedding_model: Option<String>,
    pub chunking_version: Option<String>,
    pub file_mtime: Option<i64>,
}

#[tauri::command]
pub fn files_get_by_path(
    state: State<'_, DbState>,
    path: String,
) -> Result<Option<FileRecord>, AppError> {
    state.with_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT file_id, file_hash, is_deleted, embedding_model, chunking_version, file_mtime
             FROM files WHERE file_path = ?",
        )?;
        let record = stmt
            .query_row([path], |row| {
                Ok(FileRecord {
                    file_id: row.get(0)?,
                    file_hash: row.get(1)?,
                    is_deleted: row.get(2)?,
                    embedding_model: row.get(3)?,
                    chunking_version: row.get(4)?,
                    file_mtime: row.get(5)?,
                })
            })
            .optional()?;
        Ok(record)
    })
}

// ── files_upsert_indexing (replaces insert + update pair) ─────────────────────
//
// A single atomic UPSERT removes the race condition where the frontend had to
// call files_get_by_path first and then decide between insert vs update.
// Returns the file_id of the affected row via RETURNING.

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertIndexingParams {
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
pub fn files_upsert_indexing(
    state: State<'_, DbState>,
    params: UpsertIndexingParams,
) -> Result<i64, AppError> {
    let _w = state.write_lock.lock();
    state.with_conn(|conn| {
        let file_id: i64 = conn.query_row(
            "INSERT INTO files (
                file_path, file_name, file_hash, file_mtime,
                embedding_model, embedding_dimension, chunking_version,
                status, is_deleted, updated_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, 'indexing', 0, ?)
             ON CONFLICT(file_path) DO UPDATE SET
                file_name       = excluded.file_name,
                file_hash       = excluded.file_hash,
                file_mtime      = excluded.file_mtime,
                embedding_model = excluded.embedding_model,
                embedding_dimension = excluded.embedding_dimension,
                chunking_version = excluded.chunking_version,
                status          = 'indexing',
                is_deleted      = 0,
                updated_at      = excluded.updated_at
             RETURNING file_id",
            rusqlite::params![
                params.clean_path,
                params.file_name,
                params.file_hash,
                params.file_mtime,
                params.embedding_model,
                params.embedding_dim,
                params.chunking_version,
                params.updated_at,
            ],
            |row| row.get(0),
        )?;
        Ok(file_id)
    })
}

// ── Status helpers ────────────────────────────────────────────────────────────

#[tauri::command]
pub fn files_mark_indexed(state: State<'_, DbState>, file_id: i64) -> Result<(), AppError> {
    state.with_conn(|conn| {
        conn.execute(
            "UPDATE files SET status = 'indexed', last_error = NULL, indexed_at = ? WHERE file_id = ?",
            rusqlite::params![now_ms(), file_id],
        )?;
        Ok(())
    })
}

#[tauri::command]
pub fn files_mark_failed(
    state: State<'_, DbState>,
    path: String,
    error: String,
) -> Result<(), AppError> {
    state.with_conn(|conn| {
        conn.execute(
            "INSERT INTO files (file_path, file_name, status, last_error, updated_at)
             VALUES (?, ?, 'failed', ?, ?)
             ON CONFLICT(file_path) DO UPDATE SET
                status     = 'failed',
                last_error = excluded.last_error,
                updated_at = excluded.updated_at",
            rusqlite::params![path.clone(), path, error, now_ms()],
        )?;
        Ok(())
    })
}

// ── files_insert_failed ───────────────────────────────────────────────────────

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
pub fn files_insert_failed(
    state: State<'_, DbState>,
    params: InsertFailedParams,
) -> Result<(), AppError> {
    state.with_conn(|conn| {
        conn.execute(
            "INSERT INTO files (file_path, file_name, file_hash, file_mtime, embedding_model, status, last_error, updated_at)
             VALUES (?, ?, ?, ?, 'unknown', 'failed', ?, ?)
             ON CONFLICT(file_path) DO UPDATE SET
                file_name   = excluded.file_name,
                file_hash   = excluded.file_hash,
                file_mtime  = excluded.file_mtime,
                status      = excluded.status,
                last_error  = excluded.last_error,
                updated_at  = excluded.updated_at",
            rusqlite::params![
                params.clean_path, params.file_name, params.file_hash,
                params.file_mtime, params.error, params.updated_at
            ],
        )?;
        Ok(())
    })
}

// ── Soft-delete / restore / rename ───────────────────────────────────────────

#[tauri::command]
pub fn files_soft_delete(
    state: State<'_, DbState>,
    path: String,
    updated_at: i64,
) -> Result<(), AppError> {
    state.with_conn(|conn| {
        conn.execute(
            "UPDATE files SET is_deleted = 1, updated_at = ? WHERE file_path = ?",
            rusqlite::params![updated_at, path],
        )?;
        Ok(())
    })
}

#[tauri::command]
pub fn files_restore(
    state: State<'_, DbState>,
    path: String,
    updated_at: i64,
) -> Result<(), AppError> {
    state.with_conn(|conn| {
        conn.execute(
            "UPDATE files SET is_deleted = 0, updated_at = ? WHERE file_path = ?",
            rusqlite::params![updated_at, path],
        )?;
        Ok(())
    })
}

#[tauri::command]
pub fn files_rename(
    state: State<'_, DbState>,
    old_path: String,
    new_path: String,
    updated_at: i64,
) -> Result<(), AppError> {
    state.with_conn(|conn| {
        // Use std::path::Path for cross-platform basename extraction (handles '\' on Windows).
        let new_name = std::path::Path::new(&new_path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(&new_path)
            .to_string();
        conn.execute(
            "UPDATE files SET file_path = ?, file_name = ?, updated_at = ? WHERE file_path = ?",
            rusqlite::params![new_path, new_name, updated_at, old_path],
        )?;
        Ok(())
    })
}

// ── Query helpers ─────────────────────────────────────────────────────────────

#[tauri::command]
pub fn files_count_chunks(state: State<'_, DbState>, path: String) -> Result<i64, AppError> {
    state.with_conn(|conn| {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM chunks c JOIN files f USING (file_id) WHERE f.file_path = ?",
            rusqlite::params![path],
            |row| row.get(0),
        )?;
        Ok(count)
    })
}

#[tauri::command]
pub fn files_hard_delete(state: State<'_, DbState>, path: String) -> Result<(), AppError> {
    state.with_conn(|conn| {
        conn.execute(
            "DELETE FROM files WHERE file_path = ?",
            rusqlite::params![path],
        )?;
        Ok(())
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileQueryRecord {
    pub file_id: i64,
    pub file_path: String,
    pub is_deleted: i64,
    pub file_mtime: Option<i64>,
    pub embedding_model: Option<String>,
    pub chunking_version: Option<String>,
}

#[tauri::command]
pub fn files_get_all(state: State<'_, DbState>) -> Result<Vec<FileQueryRecord>, AppError> {
    state.with_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT file_id, file_path, is_deleted, file_mtime, embedding_model, chunking_version FROM files",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(FileQueryRecord {
                file_id: row.get(0)?,
                file_path: row.get(1)?,
                is_deleted: row.get(2)?,
                file_mtime: row.get(3)?,
                embedding_model: row.get(4)?,
                chunking_version: row.get(5)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    })
}

#[tauri::command]
pub fn files_get_by_source_file_id(
    state: State<'_, DbState>,
    source_file_id: i64,
) -> Result<Vec<String>, AppError> {
    state.with_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT file_path FROM files WHERE source_file_id = ? AND is_deleted = 0",
        )?;
        let rows = stmt.query_map(rusqlite::params![source_file_id], |row| row.get(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    })
}

#[tauri::command]
pub fn files_insert_minimal(
    state: State<'_, DbState>,
    path: String,
    name: String,
    hash: String,
) -> Result<i64, AppError> {
    state.with_conn(|conn| {
        conn.execute(
            "INSERT INTO files (file_path, file_name, file_hash, status) VALUES (?, ?, ?, 'indexed')",
            rusqlite::params![path, name, hash],
        )?;
        Ok(conn.last_insert_rowid())
    })
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
    state.with_conn(|conn| {
        let file_id: i64 = conn.query_row(
            "INSERT INTO files (file_path, file_name, file_hash, file_mtime, source_file_id, is_deleted, status, updated_at)
             VALUES (?, ?, ?, ?, ?, 0, 'indexed', ?)
             ON CONFLICT(file_path) DO UPDATE SET
                file_hash      = excluded.file_hash,
                file_mtime     = excluded.file_mtime,
                source_file_id = excluded.source_file_id,
                is_deleted     = 0,
                status         = 'indexed',
                updated_at     = excluded.updated_at
             RETURNING file_id",
            rusqlite::params![path, name, hash, mtime, source_file_id, mtime],
            |row| row.get(0),
        )?;
        Ok(file_id)
    })
}

// ── Version control (Diffy) ──────────────────────────────────────────────────

fn now_ms_for_diff() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[tauri::command]
pub fn files_update_content_with_diff(
    state: State<'_, DbState>,
    file_id: i64,
    new_content: String,
) -> Result<(), AppError> {
    let _w = state.write_lock.lock();
    let now = now_ms_for_diff();

    state.with_conn(|conn| {
        let tx = conn.transaction()?;

        let chunks: Vec<String> = {
            let mut stmt = tx.prepare(
                "SELECT chunk_text FROM chunks WHERE file_id = ? ORDER BY chunk_index ASC",
            )?;
            let rows = stmt.query_map([file_id], |row| row.get::<_, Option<String>>(0))?;
            rows.map(|r| r.unwrap_or_default().unwrap_or_default()).collect()
        };
        let old_content = chunks.join("\n\n");

        if old_content != new_content {
            let patch = diffy::create_patch(&old_content, &new_content);
            let patch_text = patch.to_string();

            if !patch_text.is_empty() {
                tx.execute(
                    "INSERT INTO files_history (file_id, patch, created_at) VALUES (?, ?, ?)",
                    rusqlite::params![file_id, patch_text, now],
                )?;
            }
        }

        tx.commit()?;
        Ok(())
    })
}
