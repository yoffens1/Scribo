//! # Fragments Repository
//!
//! CRUD and search operations for `fragments` rows.

pub mod keyword;
pub mod vector;

pub use keyword::{clean_fts_query, search};
pub use vector::{bytes_to_f32_slice, vector_search};

use rusqlite::{Connection, OptionalExtension};
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

/// Deletes all fragments belonging to `note_id`. Used before a full re-index.
pub fn delete_by_note_id(conn: &Connection, note_id: i64) -> Result<i64, AppError> {
    let deleted = conn.execute(
        "DELETE FROM fragments WHERE note_id = ?",
        rusqlite::params![note_id],
    )?;
    Ok(deleted as i64)
}

/// Deletes a single fragment by its `fragment_id`.
pub fn delete_by_id(conn: &Connection, id: i64) -> Result<(), AppError> {
    conn.execute(
        "DELETE FROM fragments WHERE fragment_id = ?",
        rusqlite::params![id],
    )?;
    Ok(())
}

/// Batch-inserts multiple fragments for a note in a single transaction.
/// Silently skips rows whose `clean_hash` has already been inserted for this note
/// (in-process dedup, backed by the `idx_fragments_note_leaf_hash` unique index).
pub fn insert(conn: &mut Connection, note_id: i64, rows: Vec<FragmentInsertRow>) -> Result<(), AppError> {
    let tx = conn.transaction()?;
    {
        let mut stmt = tx.prepare(
            "INSERT OR IGNORE INTO fragments (note_id, order_index, raw_text, raw_text_hash, clean_text, clean_text_hash)
             VALUES (?, ?, ?, ?, ?, ?)",
        )?;
        let mut seen = std::collections::HashSet::new();
        for row in &rows {
            // Skip duplicate clean texts within this batch.
            if !seen.insert(row.clean_hash.clone()) {
                continue;
            }
            stmt.execute(rusqlite::params![
                note_id,
                row.fragment_index,
                row.text_clean,
                row.clean_hash,
                row.text_clean,
                row.clean_hash,
            ])?;

            // Only write the embedding when the row was actually inserted.
            let fragment_id = tx.last_insert_rowid();
            if fragment_id != 0 && !row.embedding.is_empty() {
                let dim = row.embedding.len() / 4;
                tx.execute(
                    "INSERT OR REPLACE INTO fragment_embeddings (fragment_id, embedding_model, embedding_model_version, dim, embedding, embedded_at)
                     VALUES (?, ?, '1', ?, ?, strftime('%s','now'))",
                    rusqlite::params![
                        fragment_id,
                        crate::constants::EMBEDDING_MODEL,
                        dim,
                        row.embedding,
                    ],
                )?;
            }
        }
    }
    tx.commit()?;
    Ok(())
}

/// Returns all fragments for `note_id`, ordered by `order_index`.
pub fn list_by_note(conn: &Connection, note_id: i64, embedding_model: &str) -> Result<Vec<Fragment>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT c.fragment_id, c.note_id, c.order_index, c.clean_text, c.clean_text_hash, ce.embedding
         FROM fragments c
         LEFT JOIN fragment_embeddings ce ON ce.fragment_id = c.fragment_id 
           AND ce.embedding_model = ?2 AND ce.embedding_model_version = '1'
         WHERE c.note_id = ?1
         ORDER BY c.order_index ASC"
    )?;
    let rows = stmt.query_map(rusqlite::params![note_id, embedding_model], |row| {
        Ok(Fragment {
            id: FragmentId(row.get(0)?),
            note_id: NoteId(row.get(1)?),
            fragment_index: row.get(2)?,
            text_clean: row.get(3)?,
            clean_hash: row.get(4)?,
            embedding: row.get(5)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Updates an existing fragment's clean text and hash.
/// If `clear_embedding` is `true`, deletes the embedding from `fragment_embeddings` so it needs refreshing.
pub fn update(
    conn: &Connection,
    note_id: i64,
    index: i64,
    text_raw: &str,
    raw_hash: &str,
    text_clean: &str,
    clean_hash: &str,
    clear_embedding: bool,
) -> Result<(), AppError> {
    if clear_embedding {
        conn.execute(
            "DELETE FROM fragment_embeddings 
             WHERE fragment_id = (SELECT fragment_id FROM fragments WHERE note_id = ?1 AND order_index = ?2)",
            rusqlite::params![note_id, index],
        )?;
    }
    conn.execute(
        "UPDATE fragments 
         SET raw_text = ?, raw_text_hash = ?, clean_text = ?, clean_text_hash = ?
         WHERE note_id = ? AND order_index = ?",
        rusqlite::params![text_raw, raw_hash, text_clean, clean_hash, note_id, index],
    )?;
    Ok(())
}

/// Inserts a single fragment row. Returns the new `fragment_id`.
/// `embedding` may be an empty slice when the embedding has not been computed yet.
pub fn insert_single(
    conn: &Connection,
    note_id: i64,
    index: i64,
    text_raw: &str,
    raw_hash: &str,
    text_clean: &str,
    clean_hash: &str,
    embedding: &[u8],
) -> Result<i64, AppError> {
    let rows_changed = conn.execute(
        "INSERT OR IGNORE INTO fragments (note_id, order_index, raw_text, raw_text_hash, clean_text, clean_text_hash)
         VALUES (?, ?, ?, ?, ?, ?)",
        rusqlite::params![note_id, index, text_raw, raw_hash, text_clean, clean_hash],
    )?;

    // When OR IGNORE fires the row already exists — look up the existing id.
    let fragment_id = if rows_changed > 0 {
        conn.last_insert_rowid()
    } else {
        conn.query_row(
            "SELECT fragment_id FROM fragments WHERE note_id = ? AND clean_text_hash = ? LIMIT 1",
            rusqlite::params![note_id, clean_hash],
            |r| r.get(0),
        ).unwrap_or(0)
    };

    if rows_changed > 0 && !embedding.is_empty() {
        let dim = embedding.len() / 4;
        conn.execute(
            "INSERT OR REPLACE INTO fragment_embeddings (fragment_id, embedding_model, embedding_model_version, dim, embedding, embedded_at)
             VALUES (?, ?, '1', ?, ?, strftime('%s','now'))",
            rusqlite::params![fragment_id, crate::constants::EMBEDDING_MODEL, dim, embedding],
        )?;
    }

    Ok(fragment_id)
}

/// Writes the embedding blob for a specific fragment identified by `(note_id, order_index)`.
/// Called by the embedding pipeline after `insert_single` has created the row.
pub fn set_embedding(
    conn: &Connection,
    note_id: i64,
    index: i64,
    embedding: &[u8],
    embedding_model: &str,
    embedding_model_version: &str,
) -> Result<(), AppError> {
    let fragment_id: i64 = conn.query_row(
        "SELECT fragment_id FROM fragments WHERE note_id = ? AND order_index = ?",
        rusqlite::params![note_id, index],
        |r| r.get(0),
    )?;

    let dim = embedding.len() / 4;
    conn.execute(
        "INSERT OR REPLACE INTO fragment_embeddings (fragment_id, embedding_model, embedding_model_version, dim, embedding, embedded_at)
         VALUES (?, ?, ?, ?, ?, strftime('%s','now'))",
        rusqlite::params![fragment_id, embedding_model, embedding_model_version, dim, embedding],
    )?;
    Ok(())
}

/// Returns fragments joined with their parent note metadata.
/// Optionally filtered by `note_id`; optionally includes soft-deleted notes.
pub fn list_fragments_with_note(
    conn: &Connection,
    filter_note_id: Option<i64>,
    include_deleted: bool,
    embedding_model: &str,
) -> Result<Vec<FragmentWithNote>, AppError> {
    let mut sql = "SELECT frag.fragment_id, n.path_cached, frag.order_index, frag.clean_text, frag.clean_text_hash, ce.embedding, frag.note_id, n.title
                   FROM fragments frag
                   JOIN notes n ON n.note_id = frag.note_id
                   LEFT JOIN fragment_embeddings ce ON ce.fragment_id = frag.fragment_id
                     AND ce.embedding_model = ?1 AND ce.embedding_model_version = '1'".to_string();

    let mut conditions = Vec::new();
    let mut params: Vec<&dyn rusqlite::types::ToSql> = vec![&embedding_model];

    if !include_deleted {
        conditions.push("n.lifecycle != 'deleted'");
    }
    if let Some(ref note_id) = filter_note_id {
        conditions.push("n.note_id = ?2");
        params.push(note_id);
    }

    if !conditions.is_empty() {
        sql.push_str(" AND ");
        sql.push_str(&conditions.join(" AND "));
    }

    sql.push_str(" ORDER BY n.note_id, frag.order_index");

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(params), |row| {
        Ok(FragmentWithNote {
            fragment: Fragment {
                id: FragmentId(row.get(0)?),
                note_id: NoteId(row.get(6)?),
                fragment_index: row.get(2)?,
                text_clean: row.get(3)?,
                clean_hash: row.get(4)?,
                embedding: row.get(5)?,
            },
            note_path: row.get(1)?,
            note_title: row.get(7)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Fetches up to `limit` clean fragment texts to detect the dominant vault language.
pub fn get_sample_texts(conn: &Connection, limit: i64) -> Result<Vec<String>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT clean_text FROM fragments 
         WHERE clean_text IS NOT NULL AND clean_text != ''
         LIMIT ?"
    )?;
    let rows = stmt.query_map([limit], |row| row.get::<_, String>(0))?;
    let mut res = Vec::new();
    for r in rows {
        res.push(r?);
    }
    Ok(res)
}

/// Helper to find a fragment by its ID and return it as a Section domain object.
/// This allows existing cards (which references sections) to render seamlessly.
pub fn find_as_section(conn: &Connection, id: crate::domain::section::SectionId) -> Result<Option<crate::domain::section::Section>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT fragment_id, note_id, order_index, raw_text, raw_text_hash, clean_text_hash
         FROM fragments WHERE fragment_id = ?"
    )?;
    let res = stmt.query_row([id.0], |row| {
        Ok(crate::domain::section::Section {
            id,
            note_id: NoteId(row.get(1)?),
            section_index: row.get(2)?,
            text_raw: row.get(3)?,
            heading: None,
            heading_level: None,
            raw_hash: row.get(4)?,
            clean_hash: row.get(5)?,
            content_offset_start: 0,
            content_offset_end: 0,
        })
    }).optional()?;
    Ok(res)
}
