use rusqlite::{Connection, OptionalExtension};
use crate::AppError;

pub struct ValidationResult {
    pub should_index: bool,
    pub existing_note_id: Option<i64>,
}

pub fn check_needs_indexing(
    conn: &Connection,
    file_path: &str,
    file_hash: &str,
    embedding_model: &str,
    indexing_version: &str,
    file_mtime: Option<i64>,
) -> Result<ValidationResult, AppError> {
    let query = "
        SELECT note_id, file_hash, embedding_model, indexing_version, indexing_status, file_mtime
        FROM notes
        WHERE file_path = ?1 AND is_deleted = 0
    ";

    let row = conn.query_row(query, [file_path], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, Option<i64>>(5)?,
        ))
    }).optional().map_err(|e| AppError::Other(e.to_string()))?;

    match row {
        Some((note_id, db_hash, db_model, db_indexing_ver, status, db_mtime)) => {
            if status.as_deref() == Some("failed") {
                return Ok(ValidationResult { should_index: true, existing_note_id: Some(note_id) });
            }

            // Fast path: mtime match
            if let (Some(mtime), Some(db_m)) = (file_mtime, db_mtime) {
                if mtime == db_m && db_hash.as_deref() == Some(file_hash) && db_model.as_deref() == Some(embedding_model) && db_indexing_ver.as_deref() == Some(indexing_version) {
                    return Ok(ValidationResult { should_index: false, existing_note_id: Some(note_id) });
                }
            }

            // Fallback: check actual changes
            if db_hash.as_deref() == Some(file_hash) && db_model.as_deref() == Some(embedding_model) && db_indexing_ver.as_deref() == Some(indexing_version) {
                return Ok(ValidationResult { should_index: false, existing_note_id: Some(note_id) });
            }

            Ok(ValidationResult { should_index: true, existing_note_id: Some(note_id) })
        }
        None => {
            Ok(ValidationResult { should_index: true, existing_note_id: None })
        }
    }
}
