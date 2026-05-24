use rusqlite::Connection;
use crate::AppError;

pub struct IndexingPayload<'a> {
    pub file_path: &'a str,
    pub file_name: &'a str,
    pub file_hash: &'a str,
    pub mtime: Option<i64>,
    pub embedding_model: &'a str,
    pub embedding_dim: u32,
    pub fragmenting_version: &'a str,
    pub fragments: Vec<FragmentInsertData<'a>>,
}

pub struct FragmentInsertData<'a> {
    pub fragment_index: usize,
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
        "INSERT INTO notes (file_path, file_name, file_hash, mtime, embedding_model, fragmenting_version, indexing_status, indexed_at, is_deleted)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'indexed', ?7, 0)
         ON CONFLICT(file_path) DO UPDATE SET
            file_name=excluded.file_name,
            file_hash=excluded.file_hash,
            mtime=excluded.mtime,
            embedding_model=excluded.embedding_model,
            fragmenting_version=excluded.fragmenting_version,
            indexing_status='indexed',
            indexed_at=excluded.indexed_at,
            is_deleted=0,
            indexing_error=NULL",
        (
            payload.file_path,
            payload.file_name,
            payload.file_hash,
            payload.mtime,
            payload.embedding_model,
            payload.fragmenting_version,
            now,
        ),
    ).map_err(|e| AppError::Other(e.to_string()))?;

    let note_id: i64 = tx.query_row(
        "SELECT note_id FROM notes WHERE file_path = ?1",
        [payload.file_path],
        |row| row.get(0)
    ).map_err(|e| AppError::Other(e.to_string()))?;

    // 2. Clear old fragments
    tx.execute("DELETE FROM fragments WHERE note_id = ?1", [note_id])
        .map_err(|e| AppError::Other(e.to_string()))?;

    // 3. Insert new fragments
    let mut stmt = tx.prepare(
        "INSERT INTO fragments (note_id, fragment_index, text, embedding) VALUES (?1, ?2, ?3, ?4)"
    ).map_err(|e| AppError::Other(e.to_string()))?;

    for fragment in payload.fragments {
        let embedding_bytes = bytemuck::cast_slice(&fragment.embedding);
        stmt.execute((
            note_id,
            fragment.fragment_index as i64,
            fragment.text,
            embedding_bytes,
        )).map_err(|e| AppError::Other(e.to_string()))?;
    }
    drop(stmt);

    // 4. Upsert Cards (if this is for spaced repetition)
    tx.execute(
        "INSERT OR IGNORE INTO cards (note_id) VALUES (?1)",
        [note_id],
    ).map_err(|e| AppError::Other(e.to_string()))?;
    
    let card_id: i64 = tx.last_insert_rowid();
    tx.execute(
        "INSERT OR IGNORE INTO schedules (target_type, target_id, state) VALUES ('card', ?1, 'new')",
        [card_id],
    ).map_err(|e| AppError::Other(e.to_string()))?;

    tx.commit().map_err(|e| AppError::Other(e.to_string()))?;

    Ok(note_id)
}
