use rusqlite::{Connection, OptionalExtension};
use crate::error::AppError;
use crate::domain::note::{Note, UpsertIndexingParams, InsertFailedParams, FileQueryRecord};

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}



pub fn get_by_path(conn: &Connection, path: &str) -> Result<Option<Note>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT note_id, file_hash, is_deleted, embedding_model, chunking_version, file_mtime
         FROM notes WHERE file_path = ?",
    )?;
    let record = stmt
        .query_row([path], |row| {
            Ok(Note {
                id: crate::domain::note::NoteId(row.get(0)?),
                title: String::new(),
                content: String::new(),
                tags: None,
                file_path: Some(path.to_string()),
                file_name: None,
                file_hash: row.get(1)?,
                file_mtime: row.get(5)?,
                indexing_status: crate::domain::note::IndexingStatus::Indexed,
                indexing_error: None,
                indexed_at: None,
                embedding_model: row.get(3)?,
                embedding_dimension: None,
                fragmenting_version: row.get(4)?,
                is_archived: false,
                is_deleted: row.get::<_, i64>(2).unwrap_or(0) != 0,
                created_at: 0,
                updated_at: 0,
            })
        })
        .optional()?;
    Ok(record)
}



pub fn upsert_indexing(conn: &mut Connection, params: UpsertIndexingParams) -> Result<i64, AppError> {
    let note_id: i64 = conn.query_row(
        "INSERT INTO notes (
            file_path, file_name, file_hash, file_mtime,
            embedding_model, embedding_dimension, fragmenting_version,
            status, is_deleted, updated_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, 'indexing', 0, ?)
         ON CONFLICT(file_path) DO UPDATE SET
            file_name       = excluded.file_name,
            file_hash       = excluded.file_hash,
            file_mtime      = excluded.file_mtime,
            embedding_model = excluded.embedding_model,
            embedding_dimension = excluded.embedding_dimension,
            fragmenting_version = excluded.fragmenting_version,
            status          = 'indexing',
            is_deleted      = 0,
            updated_at      = excluded.updated_at
         RETURNING note_id",
        rusqlite::params![
            params.clean_path,
            params.file_name,
            params.file_hash,
            params.file_mtime,
            params.embedding_model,
            params.embedding_dim,
            params.chunking_version, // Keep this one as the params struct uses chunking_version still
            params.updated_at,
        ],
        |row| row.get(0),
    )?;
    Ok(note_id)
}

pub fn mark_indexed(conn: &Connection, note_id: i64) -> Result<(), AppError> {
    conn.execute(
        "UPDATE notes SET status = 'indexed', last_error = NULL, indexed_at = ? WHERE note_id = ?",
        rusqlite::params![now_ms(), note_id],
    )?;
    Ok(())
}

pub fn mark_failed(conn: &Connection, path: &str, error: &str) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO notes (file_path, file_name, status, last_error, updated_at)
         VALUES (?, ?, 'failed', ?, ?)
         ON CONFLICT(file_path) DO UPDATE SET
            status     = 'failed',
            last_error = excluded.last_error,
            updated_at = excluded.updated_at",
        rusqlite::params![path, path, error, now_ms()],
    )?;
    Ok(())
}



pub fn insert_failed(conn: &Connection, params: InsertFailedParams) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO notes (file_path, file_name, file_hash, file_mtime, embedding_model, status, last_error, updated_at)
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
}

pub fn soft_delete(conn: &Connection, path: &str, updated_at: i64) -> Result<(), AppError> {
    conn.execute(
        "UPDATE notes SET is_deleted = 1, updated_at = ? WHERE file_path = ?",
        rusqlite::params![updated_at, path],
    )?;
    Ok(())
}

pub fn restore(conn: &Connection, path: &str, updated_at: i64) -> Result<(), AppError> {
    conn.execute(
        "UPDATE notes SET is_deleted = 0, updated_at = ? WHERE file_path = ?",
        rusqlite::params![updated_at, path],
    )?;
    Ok(())
}

pub fn rename(conn: &Connection, old_path: &str, new_path: &str, updated_at: i64) -> Result<(), AppError> {
    let new_name = std::path::Path::new(new_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(new_path)
        .to_string();
    conn.execute(
        "UPDATE notes SET file_path = ?, file_name = ?, updated_at = ? WHERE file_path = ?",
        rusqlite::params![new_path, new_name, updated_at, old_path],
    )?;
    Ok(())
}

pub fn count_fragments(conn: &Connection, path: &str) -> Result<i64, AppError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM fragments c JOIN notes f USING (note_id) WHERE f.file_path = ?",
        rusqlite::params![path],
        |row| row.get(0),
    )?;
    Ok(count)
}

pub fn hard_delete(conn: &Connection, path: &str) -> Result<(), AppError> {
    conn.execute(
        "DELETE FROM notes WHERE file_path = ?",
        rusqlite::params![path],
    )?;
    Ok(())
}



pub fn get_all(conn: &Connection) -> Result<Vec<FileQueryRecord>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT note_id, file_path, is_deleted, file_mtime, embedding_model, chunking_version FROM notes",
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
}

pub fn get_by_source_note_id(conn: &Connection, source_note_id: i64) -> Result<Vec<String>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT file_path FROM notes WHERE source_note_id = ? AND is_deleted = 0",
    )?;
    let rows = stmt.query_map(rusqlite::params![source_note_id], |row| row.get(0))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn insert_minimal(conn: &Connection, path: &str, name: &str, hash: &str) -> Result<i64, AppError> {
    conn.execute(
        "INSERT INTO notes (file_path, file_name, file_hash, status) VALUES (?, ?, ?, 'indexed')",
        rusqlite::params![path, name, hash],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn sync_upsert(
    conn: &mut Connection,
    path: &str,
    name: &str,
    hash: &str,
    mtime: i64,
    source_note_id: Option<i64>,
) -> Result<i64, AppError> {
    let note_id: i64 = conn.query_row(
        "INSERT INTO notes (file_path, file_name, file_hash, file_mtime, source_note_id, is_deleted, status, updated_at)
         VALUES (?, ?, ?, ?, ?, 0, 'indexed', ?)
         ON CONFLICT(file_path) DO UPDATE SET
            file_hash      = excluded.file_hash,
            file_mtime     = excluded.file_mtime,
            source_note_id = excluded.source_note_id,
            is_deleted     = 0,
            status         = 'indexed',
            updated_at     = excluded.updated_at
         RETURNING note_id",
        rusqlite::params![path, name, hash, mtime, source_note_id, mtime],
        |row| row.get(0),
    )?;
    Ok(note_id)
}

fn now_ms_for_diff() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn update_content_with_diff(
    conn: &mut Connection,
    note_id: i64,
    new_content: &str,
) -> Result<(), AppError> {
    let now = now_ms_for_diff();
    let tx = conn.transaction()?;

    let fragments: Vec<String> = {
        let mut stmt = tx.prepare(
            "SELECT text FROM fragments WHERE note_id = ? ORDER BY fragment_index ASC",
        )?;
        let rows = stmt.query_map([note_id], |row| row.get::<_, Option<String>>(0))?;
        rows.map(|r| r.unwrap_or_default().unwrap_or_default()).collect()
    };
    let old_content = fragments.join("\n\n");

    if old_content != new_content {
        let patch = diffy::create_patch(&old_content, new_content);
        let patch_text = patch.to_string();

        if !patch_text.is_empty() {
            tx.execute(
                "INSERT INTO note_revisions (note_id, patch, created_at) VALUES (?, ?, ?)",
                rusqlite::params![note_id, patch_text, now],
            )?;
        }
    }

    tx.commit()?;
    Ok(())
}
