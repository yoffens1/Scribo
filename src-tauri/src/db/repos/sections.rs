use rusqlite::Connection;
use crate::error::AppError;
use crate::domain::section::{Section, SectionId};
use crate::domain::note::NoteId;

pub fn delete_by_note_id(conn: &Connection, note_id: i64) -> Result<i64, AppError> {
    let deleted = conn.execute(
        "DELETE FROM chunks WHERE note_id = ? AND level = 0",
        rusqlite::params![note_id],
    )?;
    Ok(deleted as i64)
}

pub fn delete_by_id(conn: &Connection, id: i64) -> Result<(), AppError> {
    conn.execute(
        "DELETE FROM chunks WHERE chunk_id = ? AND level = 0",
        rusqlite::params![id],
    )?;
    Ok(())
}

pub fn insert_single(
    conn: &Connection,
    note_id: i64,
    index: i64,
    text_raw: &str,
    heading: Option<&str>,
    heading_level: Option<i64>,
    raw_hash: &str,
    clean_hash: &str,
    content_offset_start: i64,
    content_offset_end: i64,
) -> Result<i64, AppError> {
    conn.execute(
        "INSERT INTO chunks (note_id, level, order_index, raw_text, raw_text_hash, clean_text, clean_text_hash, heading, heading_level, content_offset_start, content_offset_end, kind)
         VALUES (?, 0, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'heading_block')",
        rusqlite::params![
            note_id,
            index,
            text_raw,
            raw_hash,
            text_raw,
            clean_hash,
            heading,
            heading_level,
            content_offset_start,
            content_offset_end
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update(
    conn: &Connection,
    section_id: i64,
    text_raw: &str,
    heading: Option<&str>,
    heading_level: Option<i64>,
    raw_hash: &str,
    clean_hash: &str,
    content_offset_start: i64,
    content_offset_end: i64,
) -> Result<(), AppError> {
    conn.execute(
        "UPDATE chunks 
         SET raw_text = ?, raw_text_hash = ?, clean_text = ?, clean_text_hash = ?, heading = ?, heading_level = ?, content_offset_start = ?, content_offset_end = ? 
         WHERE chunk_id = ? AND level = 0",
        rusqlite::params![
            text_raw,
            raw_hash,
            text_raw,
            clean_hash,
            heading,
            heading_level,
            content_offset_start,
            content_offset_end,
            section_id
        ],
    )?;
    Ok(())
}

pub fn list_by_note(conn: &Connection, note_id: i64) -> Result<Vec<Section>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT chunk_id, note_id, order_index, raw_text, heading, heading_level, raw_text_hash, clean_text_hash, content_offset_start, content_offset_end 
         FROM chunks WHERE note_id = ? AND level = 0 ORDER BY order_index ASC"
    )?;
    let rows = stmt.query_map([note_id], |row| {
        Ok(Section {
            id: SectionId(row.get(0)?),
            note_id: NoteId(row.get(1)?),
            section_index: row.get(2)?,
            text_raw: row.get(3)?,
            heading: row.get(4)?,
            heading_level: row.get(5)?,
            raw_hash: row.get(6)?,
            clean_hash: row.get(7)?,
            content_offset_start: row.get(8)?,
            content_offset_end: row.get(9)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn find_by_id(conn: &Connection, id: SectionId) -> Result<Option<Section>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT chunk_id, note_id, order_index, raw_text, heading, heading_level, raw_text_hash, clean_text_hash, content_offset_start, content_offset_end 
         FROM chunks WHERE chunk_id = ? AND level = 0"
    )?;
    let mut rows = stmt.query_map([id.0], |row| {
        Ok(Section {
            id: SectionId(row.get(0)?),
            note_id: NoteId(row.get(1)?),
            section_index: row.get(2)?,
            text_raw: row.get(3)?,
            heading: row.get(4)?,
            heading_level: row.get(5)?,
            raw_hash: row.get(6)?,
            clean_hash: row.get(7)?,
            content_offset_start: row.get(8)?,
            content_offset_end: row.get(9)?,
        })
    })?;
    if let Some(row) = rows.next() {
        Ok(Some(row?))
    } else {
        Ok(None)
    }
}
