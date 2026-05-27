use rusqlite::Connection;
use crate::error::AppError;
use crate::domain::section::{Section, SectionId};
use crate::domain::note::NoteId;

pub fn delete_by_note_id(conn: &Connection, note_id: i64) -> Result<i64, AppError> {
    let deleted = conn.execute(
        "DELETE FROM sections WHERE note_id = ?",
        rusqlite::params![note_id],
    )?;
    Ok(deleted as i64)
}

pub fn delete_by_id(conn: &Connection, id: i64) -> Result<(), AppError> {
    conn.execute(
        "DELETE FROM sections WHERE section_id = ?",
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
    source_hash: &str,
    content_offset_start: i64,
    content_offset_end: i64,
) -> Result<i64, AppError> {
    conn.execute(
        "INSERT INTO sections (note_id, section_index, text_raw, heading, heading_level, source_hash, content_offset_start, content_offset_end)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        rusqlite::params![note_id, index, text_raw, heading, heading_level, source_hash, content_offset_start, content_offset_end],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update(
    conn: &Connection,
    section_id: i64,
    text_raw: &str,
    heading: Option<&str>,
    heading_level: Option<i64>,
    source_hash: &str,
    content_offset_start: i64,
    content_offset_end: i64,
) -> Result<(), AppError> {
    conn.execute(
        "UPDATE sections 
         SET text_raw = ?, heading = ?, heading_level = ?, source_hash = ?, content_offset_start = ?, content_offset_end = ? 
         WHERE section_id = ?",
        rusqlite::params![text_raw, heading, heading_level, source_hash, content_offset_start, content_offset_end, section_id],
    )?;
    Ok(())
}

pub fn list_by_note(conn: &Connection, note_id: i64) -> Result<Vec<Section>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT section_id, note_id, section_index, text_raw, heading, heading_level, source_hash, content_offset_start, content_offset_end 
         FROM sections WHERE note_id = ? ORDER BY section_index ASC"
    )?;
    let rows = stmt.query_map([note_id], |row| {
        Ok(Section {
            id: SectionId(row.get(0)?),
            note_id: NoteId(row.get(1)?),
            section_index: row.get(2)?,
            text_raw: row.get(3)?,
            heading: row.get(4)?,
            heading_level: row.get(5)?,
            source_hash: row.get(6)?,
            content_offset_start: row.get(7)?,
            content_offset_end: row.get(8)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn find_by_id(conn: &Connection, id: SectionId) -> Result<Option<Section>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT section_id, note_id, section_index, text_raw, heading, heading_level, source_hash, content_offset_start, content_offset_end 
         FROM sections WHERE section_id = ?"
    )?;
    let mut rows = stmt.query_map([id.0], |row| {
        Ok(Section {
            id: SectionId(row.get(0)?),
            note_id: NoteId(row.get(1)?),
            section_index: row.get(2)?,
            text_raw: row.get(3)?,
            heading: row.get(4)?,
            heading_level: row.get(5)?,
            source_hash: row.get(6)?,
            content_offset_start: row.get(7)?,
            content_offset_end: row.get(8)?,
        })
    })?;
    if let Some(row) = rows.next() {
        Ok(Some(row?))
    } else {
        Ok(None)
    }
}
