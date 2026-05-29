//! # Fragments Repository
//!
//! CRUD and search operations for `chunks` rows at **`level = 1`** (leaf fragments).

pub mod keyword;
pub mod vector;

pub use keyword::{clean_fts_query, search};
pub use vector::{bytes_to_f32_slice, vector_search};

use rusqlite::Connection;
use crate::error::AppError;
use crate::domain::fragment::{FragmentInsertRow, Fragment, FragmentId};
use crate::domain::note::NoteId;

/// A fragment enriched with its parent note's title and filesystem path.
/// Used by list endpoints that need to display fragment context.
#[derive(Debug, Clone)]
pub struct FragmentWithNote {
    pub fragment: Fragment,
    pub note_path: Option<String>,
    pub note_title: String,
}

/// Deletes all `level = 1` chunks belonging to `note_id`. Used before a full re-index.
pub fn delete_by_note_id(conn: &Connection, note_id: i64) -> Result<i64, AppError> {
    let deleted = conn.execute(
        "DELETE FROM chunks WHERE note_id = ? AND level = 1",
        rusqlite::params![note_id],
    )?;
    Ok(deleted as i64)
}

/// Deletes a single fragment by its `chunk_id`. Level guard prevents accidental section deletion.
pub fn delete_by_id(conn: &Connection, id: i64) -> Result<(), AppError> {
    conn.execute(
        "DELETE FROM chunks WHERE chunk_id = ? AND level = 1",
        rusqlite::params![id],
    )?;
    Ok(())
}

/// Batch-inserts multiple fragments for a note in a single transaction.
/// Used by the legacy bulk-import path.
pub fn insert(conn: &mut Connection, note_id: i64, rows: Vec<FragmentInsertRow>) -> Result<(), AppError> {
    let tx = conn.transaction()?;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO chunks (note_id, level, order_index, raw_text, raw_text_hash, clean_text, clean_text_hash, token_count, embedding, kind)
             VALUES (?, 1, ?, ?, ?, ?, ?, ?, ?, 'fragment')",
        )?;
        for row in &rows {
            stmt.execute(rusqlite::params![
                note_id,
                row.fragment_index,
                row.text_clean,
                row.clean_hash,
                row.text_clean,
                row.clean_hash,
                row.token_count,
                row.embedding
            ])?;
        }
    }
    tx.commit()?;
    Ok(())
}

/// Returns all `level = 1` fragments for `note_id`, ordered by `order_index`.
pub fn list_by_note(conn: &Connection, note_id: i64) -> Result<Vec<Fragment>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT chunk_id, note_id, order_index, clean_text, clean_text_hash, token_count, embedding, parent_chunk_id
         FROM chunks WHERE note_id = ? AND level = 1 ORDER BY order_index ASC"
    )?;
    let rows = stmt.query_map([note_id], |row| {
        let parent_chunk_id: Option<i64> = row.get(7)?;
        Ok(Fragment {
            id: FragmentId(row.get(0)?),
            note_id: NoteId(row.get(1)?),
            section_id: parent_chunk_id.map(crate::domain::SectionId),
            fragment_index: row.get(2)?,
            text_clean: row.get(3)?,
            clean_hash: row.get(4)?,
            token_count: row.get(5)?,
            embedding: row.get(6)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Updates an existing fragment's clean text and hash.
/// If `clear_embedding` is `true`, sets `embedding = NULL` so the embedder knows it needs refreshing.
pub fn update(
    conn: &Connection,
    note_id: i64,
    index: i64,
    text_clean: &str,
    clean_hash: &str,
    clear_embedding: bool,
    parent_chunk_id: Option<i64>,
) -> Result<(), AppError> {
    if clear_embedding {
        conn.execute(
            "UPDATE chunks 
             SET raw_text = ?, raw_text_hash = ?, clean_text = ?, clean_text_hash = ?, embedding = NULL, parent_chunk_id = ? 
             WHERE note_id = ? AND order_index = ? AND level = 1",
            rusqlite::params![text_clean, clean_hash, text_clean, clean_hash, parent_chunk_id, note_id, index],
        )?;
    } else {
        conn.execute(
            "UPDATE chunks 
             SET raw_text = ?, raw_text_hash = ?, clean_text = ?, clean_text_hash = ?, parent_chunk_id = ? 
             WHERE note_id = ? AND order_index = ? AND level = 1",
            rusqlite::params![text_clean, clean_hash, text_clean, clean_hash, parent_chunk_id, note_id, index],
        )?;
    }
    Ok(())
}

/// Inserts a single fragment row. Returns the new `chunk_id`.
/// `embedding` may be an empty slice when the embedding has not been computed yet.
pub fn insert_single(
    conn: &Connection,
    note_id: i64,
    index: i64,
    text_clean: &str,
    clean_hash: &str,
    token_count: Option<i64>,
    embedding: &[u8],
    parent_chunk_id: Option<i64>,
) -> Result<i64, AppError> {
    conn.execute(
        "INSERT INTO chunks (note_id, level, order_index, raw_text, raw_text_hash, clean_text, clean_text_hash, token_count, embedding, parent_chunk_id, kind)
         VALUES (?, 1, ?, ?, ?, ?, ?, ?, ?, ?, 'fragment')",
        rusqlite::params![note_id, index, text_clean, clean_hash, text_clean, clean_hash, token_count, embedding, parent_chunk_id],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Writes the embedding blob for a specific fragment identified by `(note_id, order_index)`.
/// Called by the embedding pipeline after `insert_single` has created the row.
pub fn set_embedding(
    conn: &Connection,
    note_id: i64,
    index: i64,
    embedding: &[u8],
) -> Result<(), AppError> {
    conn.execute(
        "UPDATE chunks SET embedding = ? WHERE note_id = ? AND order_index = ? AND level = 1",
        rusqlite::params![embedding, note_id, index],
    )?;
    Ok(())
}

/// Returns fragments joined with their parent note metadata.
/// Optionally filtered by `note_id`; optionally includes soft-deleted notes.
pub fn list_fragments_with_note(
    conn: &Connection,
    filter_note_id: Option<i64>,
    include_deleted: bool,
) -> Result<Vec<FragmentWithNote>, AppError> {
    let mut sql = "SELECT frag.chunk_id, n.path_cached, frag.order_index, frag.clean_text, frag.clean_text_hash, frag.token_count, frag.embedding, frag.note_id, n.title, frag.parent_chunk_id
                   FROM chunks frag
                   JOIN notes n ON n.note_id = frag.note_id
                   WHERE frag.level = 1".to_string();

    let mut conditions = Vec::new();
    let mut params: Vec<&dyn rusqlite::types::ToSql> = Vec::new();

    if !include_deleted {
        conditions.push("n.lifecycle != 'deleted'");
    }
    if let Some(ref note_id) = filter_note_id {
        conditions.push("n.note_id = ?");
        params.push(note_id);
    }

    if !conditions.is_empty() {
        sql.push_str(" AND ");
        sql.push_str(&conditions.join(" AND "));
    }

    sql.push_str(" ORDER BY n.note_id, frag.order_index");

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(params), |row| {
        let parent_chunk_id: Option<i64> = row.get(9)?;
        Ok(FragmentWithNote {
            fragment: Fragment {
                id: FragmentId(row.get(0)?),
                note_id: NoteId(row.get(7)?),
                section_id: parent_chunk_id.map(crate::domain::SectionId),
                fragment_index: row.get(2)?,
                text_clean: row.get(3)?,
                clean_hash: row.get(4)?,
                token_count: row.get(5)?,
                embedding: row.get(6)?,
            },
            note_path: row.get(1)?,
            note_title: row.get(8)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Fetches up to `limit` clean fragment texts to detect the dominant vault language.
pub fn get_sample_texts(conn: &Connection, limit: i64) -> Result<Vec<String>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT clean_text FROM chunks 
         WHERE level = 1 AND clean_text IS NOT NULL AND clean_text != ''
         LIMIT ?"
    )?;
    let rows = stmt.query_map([limit], |row| row.get::<_, String>(0))?;
    let mut res = Vec::new();
    for r in rows {
        res.push(r?);
    }
    Ok(res)
}
