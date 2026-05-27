use rusqlite::{Connection, OptionalExtension};
use crate::error::AppError;
use crate::domain::note::{Note, NoteId, IndexingStatus};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NoteListItem {
    pub note_id: NoteId,
    pub title: String,
    pub is_deleted: bool,
    pub embedding_model: Option<String>,
    pub indexing_version: Option<String>,
}

fn row_to_note(row: &rusqlite::Row) -> rusqlite::Result<Note> {
    let status_str: String = row.get(5)?;
    Ok(Note {
        id: NoteId(row.get(0)?),
        title: row.get(1)?,
        content: row.get(2)?,
        content_hash: row.get(3)?,
        tags: row.get(4)?,
        indexing_status: IndexingStatus::parse(&status_str).unwrap_or(IndexingStatus::Pending),
        indexing_error: row.get(6)?,
        indexed_at: row.get(7)?,
        embedding_model: row.get(8)?,
        embedding_dimension: row.get(9)?,
        indexing_version: row.get(10)?,
        is_archived: row.get::<_, i64>(11).unwrap_or(0) != 0,
        is_deleted: row.get::<_, i64>(12).unwrap_or(0) != 0,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

const SELECT_NOTE_COLUMNS: &str = 
    "SELECT note_id, title, content, content_hash, tags, 
            indexing_status, indexing_error, indexed_at, embedding_model, embedding_dimension, 
            indexing_version, is_archived, is_deleted, created_at, updated_at
     FROM notes";

pub fn get_by_id(conn: &Connection, note_id: i64) -> Result<Option<Note>, AppError> {
    let sql = format!("{} WHERE note_id = ?", SELECT_NOTE_COLUMNS);
    let mut stmt = conn.prepare(&sql)?;
    let record = stmt.query_row([note_id], row_to_note).optional()?;
    Ok(record)
}

pub fn insert(conn: &Connection, title: &str, content: &str, tags: Option<&str>) -> Result<NoteId, AppError> {
    let now = crate::db::time::now_seconds();
    let content_hash = blake3::hash(content.as_bytes()).to_hex().to_string();

    let note_id: i64 = conn.query_row(
        "INSERT INTO notes (
            title, content, content_hash, indexing_status, tags, is_archived, is_deleted, created_at, updated_at
         ) VALUES (?, ?, ?, 'pending', ?, 0, 0, ?, ?)
         RETURNING note_id",
        rusqlite::params![
            title,
            content,
            content_hash,
            tags,
            now,
            now,
        ],
        |row| row.get(0),
    )?;
    Ok(NoteId(note_id))
}

pub fn mark_indexed(conn: &Connection, note_id: i64) -> Result<(), AppError> {
    conn.execute(
        "UPDATE notes SET indexing_status = 'indexed', indexing_error = NULL, indexed_at = ? WHERE note_id = ?",
        rusqlite::params![crate::db::time::now_seconds(), note_id],
    )?;
    Ok(())
}

pub fn record_failure(conn: &Connection, note_id: i64, error: &str) -> Result<(), AppError> {
    conn.execute(
        "UPDATE notes SET indexing_status = 'failed', indexing_error = ?, updated_at = ? WHERE note_id = ?",
        rusqlite::params![error, crate::db::time::now_seconds(), note_id],
    )?;
    Ok(())
}

pub fn soft_delete(conn: &Connection, note_id: i64, updated_at: i64) -> Result<(), AppError> {
    conn.execute(
        "UPDATE notes SET is_deleted = 1, updated_at = ? WHERE note_id = ?",
        rusqlite::params![updated_at, note_id],
    )?;
    Ok(())
}

pub fn restore(conn: &Connection, note_id: i64, updated_at: i64) -> Result<(), AppError> {
    conn.execute(
        "UPDATE notes SET is_deleted = 0, updated_at = ? WHERE note_id = ?",
        rusqlite::params![updated_at, note_id],
    )?;
    Ok(())
}

pub fn rename(conn: &Connection, note_id: i64, new_title: &str, updated_at: i64) -> Result<(), AppError> {
    conn.execute(
        "UPDATE notes SET title = ?, updated_at = ? WHERE note_id = ?",
        rusqlite::params![new_title, updated_at, note_id],
    )?;
    Ok(())
}

pub fn count_fragments(conn: &Connection, note_id: i64) -> Result<i64, AppError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM fragments WHERE note_id = ?",
        rusqlite::params![note_id],
        |row| row.get(0),
    )?;
    Ok(count)
}

pub fn hard_delete(conn: &Connection, note_id: i64) -> Result<(), AppError> {
    conn.execute(
        "DELETE FROM notes WHERE note_id = ?",
        rusqlite::params![note_id],
    )?;
    Ok(())
}

pub fn get_all(conn: &Connection) -> Result<Vec<NoteListItem>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT note_id, title, is_deleted, embedding_model, indexing_version FROM notes",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(NoteListItem {
            note_id: NoteId(row.get(0)?),
            title: row.get(1)?,
            is_deleted: row.get::<_, i64>(2)? != 0,
            embedding_model: row.get(3)?,
            indexing_version: row.get(4)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn update_content_with_diff(
    conn: &mut Connection,
    note_id: i64,
    new_content: &str,
) -> Result<(), AppError> {
    let now = crate::db::time::now_seconds();
    let tx = conn.transaction()?;

    let old_content: String = tx.query_row(
        "SELECT content FROM notes WHERE note_id = ?",
        [note_id],
        |row| row.get(0)
    ).unwrap_or_default();

    if old_content != new_content {
        let patch = diffy::create_patch(&old_content, new_content);
        let patch_text = patch.to_string();

        if !patch_text.is_empty() {
            tx.execute(
                "INSERT INTO note_revisions (note_id, patch, created_at) VALUES (?, ?, ?)",
                rusqlite::params![note_id, patch_text, now],
            )?;
        }

        let content_hash = blake3::hash(new_content.as_bytes()).to_hex().to_string();

        tx.execute(
            "UPDATE notes SET content = ?, content_hash = ?, indexing_status = 'stale', updated_at = ? WHERE note_id = ?",
            rusqlite::params![new_content, content_hash, now, note_id],
        )?;
    }

    tx.commit()?;
    Ok(())
}

pub fn set_content_hash(conn: &Connection, note_id: i64, hash: &str) -> Result<(), AppError> {
    conn.execute(
        "UPDATE notes SET content_hash = ? WHERE note_id = ?",
        rusqlite::params![hash, note_id],
    )?;
    Ok(())
}

pub fn set_status(conn: &Connection, note_id: i64, status: IndexingStatus) -> Result<(), AppError> {
    conn.execute(
        "UPDATE notes SET indexing_status = ? WHERE note_id = ?",
        rusqlite::params![status.as_str(), note_id],
    )?;
    Ok(())
}

pub fn set_content(conn: &Connection, note_id: i64, content: &str) -> Result<(), AppError> {
    let now = crate::db::time::now_seconds();
    let content_hash = blake3::hash(content.as_bytes()).to_hex().to_string();
    conn.execute(
        "UPDATE notes SET content = ?, content_hash = ?, indexing_status = 'stale', updated_at = ? WHERE note_id = ?",
        rusqlite::params![content, content_hash, now, note_id],
    )?;
    Ok(())
}
