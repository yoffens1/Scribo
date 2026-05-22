use tauri::State;
use serde::{Deserialize, Serialize};
use crate::DbState;
use crate::error::AppError;

#[tauri::command]
pub fn chunks_delete_by_file_id(state: State<'_, DbState>, file_id: i64) -> Result<i64, AppError> {
    let mut opt_conn = state.0.lock();
    let conn = opt_conn.as_mut().ok_or(AppError::NotInitialized)?;

    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM chunks WHERE file_id = ?",
        rusqlite::params![file_id],
        |row| row.get(0),
    )?;

    conn.execute("DELETE FROM chunks WHERE file_id = ?", rusqlite::params![file_id])?;
    Ok(count)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkInsertRow {
    pub chunk_index: i64,
    pub text: String,
    pub tokens: i64,
    pub embedding: String, // Base64 encoded
}

#[tauri::command]
pub fn chunks_insert(
    state: State<'_, DbState>,
    file_id: i64,
    rows: Vec<ChunkInsertRow>,
) -> Result<(), AppError> {
    let mut opt_conn = state.0.lock();
    let conn = opt_conn.as_mut().ok_or(AppError::NotInitialized)?;

    use base64::{engine::general_purpose, Engine as _};

    // We use a transaction for batch insertion performance
    let tx = conn.transaction()?;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO chunks (file_id, chunk_index, chunk_text, token_count, embedding)
             VALUES (?, ?, ?, ?, ?)"
        )?;

        for row in rows {
            let bytes = general_purpose::STANDARD.decode(row.embedding.replace("base64:", "")).map_err(|e| AppError::Other(e.to_string()))?;
            stmt.execute(rusqlite::params![
                file_id,
                row.chunk_index,
                row.text,
                row.tokens,
                bytes
            ])?;
        }
    }
    tx.commit()?;
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkDataRecord {
    pub chunk_index: i64,
    pub chunk_text: Option<String>,
    pub embedding: String, // Base64 encoded
    pub token_count: Option<i64>,
}

fn encode_blob(blob: Vec<u8>) -> String {
    use base64::{engine::general_purpose, Engine as _};
    format!("base64:{}", general_purpose::STANDARD.encode(blob))
}

#[tauri::command]
pub fn chunks_get_by_file_path(
    state: State<'_, DbState>,
    file_path: String,
    include_deleted: bool,
) -> Result<Vec<ChunkDataRecord>, AppError> {
    let mut opt_conn = state.0.lock();
    let conn = opt_conn.as_mut().ok_or(AppError::NotInitialized)?;

    let mut query = String::from(
        "SELECT c.chunk_index, c.chunk_text, c.embedding, c.token_count
         FROM chunks c
         JOIN files f ON f.file_id = c.file_id
         WHERE f.file_path = ?"
    );
    if !include_deleted {
        query.push_str(" AND f.is_deleted = 0");
    }
    query.push_str(" ORDER BY c.chunk_index");

    let mut stmt = conn.prepare(&query)?;
    let rows = stmt.query_map(rusqlite::params![file_path], |row| {
        Ok(ChunkDataRecord {
            chunk_index: row.get(0)?,
            chunk_text: row.get(1)?,
            embedding: encode_blob(row.get(2)?),
            token_count: row.get(3)?,
        })
    })?;

    let mut res = Vec::new();
    for r in rows { res.push(r?); }
    Ok(res)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FullChunkDataRecord {
    pub chunk_id: i64,
    pub file_path: String,
    pub chunk_index: i64,
    pub chunk_text: Option<String>,
    pub token_count: Option<i64>,
    pub embedding: String,
}

#[tauri::command]
pub fn chunks_get_all(
    state: State<'_, DbState>,
    include_deleted: bool,
) -> Result<Vec<FullChunkDataRecord>, AppError> {
    let mut opt_conn = state.0.lock();
    let conn = opt_conn.as_mut().ok_or(AppError::NotInitialized)?;

    let mut query = String::from(
        "SELECT c.chunk_id, f.file_path, c.chunk_index, c.chunk_text, c.token_count, c.embedding
         FROM chunks c
         JOIN files f ON f.file_id = c.file_id"
    );
    if !include_deleted {
        query.push_str(" WHERE f.is_deleted = 0");
    }
    query.push_str(" ORDER BY f.file_path, c.chunk_index");

    let mut stmt = conn.prepare(&query)?;
    let rows = stmt.query_map([], |row| {
        Ok(FullChunkDataRecord {
            chunk_id: row.get(0)?,
            file_path: row.get(1)?,
            chunk_index: row.get(2)?,
            chunk_text: row.get(3)?,
            token_count: row.get(4)?,
            embedding: encode_blob(row.get(5)?),
        })
    })?;

    let mut res = Vec::new();
    for r in rows { res.push(r?); }
    Ok(res)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkDataWithPathRecord {
    pub file_path: String,
    pub chunk_index: i64,
    pub chunk_text: Option<String>,
    pub token_count: Option<i64>,
    pub embedding: String,
}

#[tauri::command]
pub fn chunks_get_by_file_name(
    state: State<'_, DbState>,
    name: String,
    include_deleted: bool,
) -> Result<Vec<ChunkDataWithPathRecord>, AppError> {
    let mut opt_conn = state.0.lock();
    let conn = opt_conn.as_mut().ok_or(AppError::NotInitialized)?;

    let normalized = if name.ends_with(".md") { name } else { format!("{}.md", name) };

    let mut query = String::from(
        "SELECT f.file_path, c.chunk_index, c.chunk_text, c.token_count, c.embedding
         FROM chunks c
         JOIN files f ON f.file_id = c.file_id
         WHERE f.file_name = ?"
    );
    if !include_deleted {
        query.push_str(" AND f.is_deleted = 0");
    }
    query.push_str(" ORDER BY f.file_path, c.chunk_index");

    let mut stmt = conn.prepare(&query)?;
    let rows = stmt.query_map(rusqlite::params![normalized], |row| {
        Ok(ChunkDataWithPathRecord {
            file_path: row.get(0)?,
            chunk_index: row.get(1)?,
            chunk_text: row.get(2)?,
            token_count: row.get(3)?,
            embedding: encode_blob(row.get(4)?),
        })
    })?;

    let mut res = Vec::new();
    for r in rows { res.push(r?); }
    Ok(res)
}
