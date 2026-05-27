use rusqlite::{Connection, OptionalExtension};
use crate::AppError;

pub fn check_needs_indexing(
    conn: &Connection,
    note_id: i64,
    embedding_model: &str,
    indexing_version: &str,
) -> Result<bool, AppError> {
    let query = "
        SELECT embedding_model, indexing_version, indexing_status
        FROM notes
        WHERE note_id = ?1 AND is_deleted = 0
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
            if status_str == "failed" || status_str == "pending" || status_str == "stale" {
                return Ok(true);
            }
            if db_model.as_deref() != Some(embedding_model) || db_indexing_ver.as_deref() != Some(indexing_version) {
                return Ok(true);
            }
            Ok(false)
        }
        None => Ok(false),
    }
}
