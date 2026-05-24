use rusqlite::Connection;
use crate::AppError;

pub struct IndexingPayload<'a> {
    pub file_path: &'a str,
    pub file_name: &'a str,
    pub file_hash: &'a str,
    pub mtime: Option<i64>,
    pub embedding_model: &'a str,
    pub embedding_dim: u32,
    pub chunking_version: &'a str,
    pub chunks: Vec<ChunkInsertData<'a>>,
}

pub struct ChunkInsertData<'a> {
    pub chunk_index: usize,
    pub text: &'a str,
    pub embedding: Vec<f32>, // Alternatively Vec<u8> if it's already serialized
}

pub fn persist_indexed_file(
    conn: &mut Connection,
    payload: IndexingPayload,
) -> Result<i64, AppError> {
    let tx = conn.transaction().map_err(|e| AppError::Other(e.to_string()))?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    // 1. Upsert file record
    tx.execute(
        "INSERT INTO files (file_path, file_name, file_hash, mtime, embedding_model, chunking_version, status, updated_at, is_deleted)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'indexed', ?7, 0)
         ON CONFLICT(file_path) DO UPDATE SET
            file_name=excluded.file_name,
            file_hash=excluded.file_hash,
            mtime=excluded.mtime,
            embedding_model=excluded.embedding_model,
            chunking_version=excluded.chunking_version,
            status='indexed',
            updated_at=excluded.updated_at,
            is_deleted=0,
            last_error=NULL",
        (
            payload.file_path,
            payload.file_name,
            payload.file_hash,
            payload.mtime,
            payload.embedding_model,
            payload.chunking_version,
            now,
        ),
    ).map_err(|e| AppError::Other(e.to_string()))?;

    let file_id: i64 = tx.query_row(
        "SELECT file_id FROM files WHERE file_path = ?1",
        [payload.file_path],
        |row| row.get(0)
    ).map_err(|e| AppError::Other(e.to_string()))?;

    // 2. Clear old chunks
    tx.execute("DELETE FROM chunks WHERE file_id = ?1", [file_id])
        .map_err(|e| AppError::Other(e.to_string()))?;

    // 3. Insert new chunks
    let mut stmt = tx.prepare(
        "INSERT INTO chunks (file_id, chunk_index, chunk_text, embedding) VALUES (?1, ?2, ?3, ?4)"
    ).map_err(|e| AppError::Other(e.to_string()))?;

    for chunk in payload.chunks {
        let embedding_bytes = bytemuck::cast_slice(&chunk.embedding);
        stmt.execute((
            file_id,
            chunk.chunk_index as i64,
            chunk.text,
            embedding_bytes,
        )).map_err(|e| AppError::Other(e.to_string()))?;
    }
    drop(stmt);

    // 4. Upsert Cards (if this is for spaced repetition)
    tx.execute(
        "INSERT OR IGNORE INTO cards (file_id, state, reps, lapses, stability, difficulty)
         VALUES (?1, 'new', 0, 0, 0.0, 0.0)",
        [file_id],
    ).map_err(|e| AppError::Other(e.to_string()))?;

    tx.commit().map_err(|e| AppError::Other(e.to_string()))?;

    Ok(file_id)
}
