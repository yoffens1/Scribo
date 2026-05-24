use rusqlite::{Connection, Transaction};
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
    // Note: this runs AFTER migrations, so it must use the v10 schema names.
    conn.execute_batch(
        "DELETE FROM fragments WHERE note_id IN (SELECT note_id FROM notes WHERE indexing_status = 'indexing');
         UPDATE notes SET indexing_status = 'failed', indexing_error = 'Interrupted indexing' WHERE indexing_status = 'indexing';"
    )?;
    Ok(())
}

pub fn column_exists(conn: &Transaction, table: &str, column: &str) -> Result<bool, AppError> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info('{}')", table))?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == column {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn add_column_if_missing(
    conn: &Transaction,
    table: &str,
    col: &str,
    def: &str,
) -> Result<(), AppError> {
    if !column_exists(conn, table, col)? {
        conn.execute_batch(&format!("ALTER TABLE {} ADD COLUMN {} {};", table, col, def))?;
    }
    Ok(())
}
