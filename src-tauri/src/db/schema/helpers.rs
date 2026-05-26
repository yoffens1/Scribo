use rusqlite::Connection;
use crate::error::AppError;

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

pub fn recover_interrupted(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(
        "DELETE FROM fragments WHERE note_id IN (SELECT note_id FROM notes WHERE indexing_status = 'indexing');
         UPDATE notes SET indexing_status = 'failed', indexing_error = 'Interrupted indexing' WHERE indexing_status = 'indexing';"
    )?;
    Ok(())
}
