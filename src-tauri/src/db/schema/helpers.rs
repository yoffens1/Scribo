//! # Schema Helpers
//!
//! Utility functions for database health checks and crash recovery.
//! Called during `initialize_schema` before and after migration steps.

use rusqlite::Connection;
use crate::error::AppError;

/// Runs `PRAGMA integrity_check` and returns an error if the result is not `"ok"`.
/// Detects file-level corruption (torn writes, storage errors) before any migrations run.
pub fn check_integrity(conn: &Connection) -> Result<(), AppError> {
    let mut stmt = conn.prepare("PRAGMA integrity_check;")?;
    let mut rows = stmt.query([])?;

    if let Some(row) = rows.next()? {
        let val: String = row.get(0)?;
        if val != "ok" {
            return Err(AppError::Other(
                "Database corruption detected! Integrity check failed.".to_string(),
            ));
        }
    }
    Ok(())
}

/// Recovers from a crash that occurred while a note was being indexed.
/// If the app was killed mid-transaction, some notes may be stuck in `indexing_status = 'indexing'`
/// with partial chunk data. This function cleans up those orphaned chunks and marks the notes as
/// `'failed'` so the scheduler re-indexes them on the next startup.
pub fn recover_interrupted(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(
        "DELETE FROM fragments WHERE note_id IN (SELECT note_id FROM notes WHERE indexing_status = 'indexing');
         UPDATE notes SET indexing_status = 'failed', indexing_error = 'Interrupted indexing' WHERE indexing_status = 'indexing';"
    )?;
    Ok(())
}
