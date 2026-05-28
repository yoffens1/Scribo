//! # Validation Service
//!
//! Pre-flight checks that determine whether a note needs to be re-indexed before
//! the scheduler commits any database writes.
//!
//! A note needs (re-)indexing when **any** of the following is true:
//!
//! - Its `indexing_status` is `"pending"`, `"stale"`, or `"failed"`.
//! - The stored `embedding_model` does not match the currently configured model.
//! - The stored `indexing_version` does not match the current pipeline version.
//! - The note row does not exist (returns `false` — nothing to index).

use rusqlite::{Connection, OptionalExtension};
use crate::AppError;

/// Returns `true` when the note needs to be indexed or re-indexed.
///
/// Queries the note's current `embedding_model`, `indexing_version`, and `indexing_status`
/// and compares them against the provided expected values.
///
/// Soft-deleted notes (`lifecycle = 'deleted'`) are excluded and return `false`.
pub fn check_needs_indexing(
    conn: &Connection,
    note_id: i64,
    embedding_model: &str,
    indexing_version: &str,
) -> Result<bool, AppError> {
    let query = "
        SELECT embedding_model, indexing_version, indexing_status
        FROM notes
        WHERE note_id = ?1 AND lifecycle != 'deleted'
    ";

    let row = conn.query_row(query, [note_id], |row| {
        Ok((
            row.get::<_, Option<String>>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, Option<String>>(2)?,
        ))
    }).optional().map_err(|e| AppError::Other(e.to_string()))?;

    match row {
        Some((db_model, db_indexing_ver, status)) => {
            let status_str = status.as_deref().unwrap_or("pending");
            // Status-based triggers — note is in a state that requires (re-)indexing.
            if status_str == "failed" || status_str == "pending" || status_str == "stale" {
                return Ok(true);
            }
            // Model or version mismatch — pipeline has changed since last run.
            if db_model.as_deref() != Some(embedding_model) || db_indexing_ver.as_deref() != Some(indexing_version) {
                return Ok(true);
            }
            Ok(false)
        }
        // Note does not exist or is deleted — nothing to do.
        None => Ok(false),
    }
}
