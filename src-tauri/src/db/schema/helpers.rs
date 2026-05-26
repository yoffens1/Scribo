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
    // Note: this runs AFTER migrations, so it must use the v11 schema names.
    let version = super::migrations::get_schema_version(conn)?;
    if version < 11 {
        return Err(AppError::Other(format!(
            "recover_interrupted requires schema v11, got v{}", version
        )));
    }
    conn.execute_batch(
        "DELETE FROM fragments WHERE note_id IN (SELECT note_id FROM notes WHERE indexing_status = 'indexing');
         UPDATE notes SET indexing_status = 'failed', indexing_error = 'Interrupted indexing' WHERE indexing_status = 'indexing';"
    )?;
    Ok(())
}

pub fn backfill_notes_after_migration(conn: &Connection) -> Result<(), AppError> {
    // 1. Backfill title if empty from file_name or file_path
    let mut stmt = conn.prepare("SELECT note_id, file_name, file_path FROM notes WHERE title = '' OR title IS NULL")?;
    let mut rows = stmt.query([])?;
    let mut title_updates = Vec::new();
    while let Some(row) = rows.next()? {
        let id: i64 = row.get(0)?;
        let file_name: Option<String> = row.get(1)?;
        let file_path: Option<String> = row.get(2)?;
        
        let display_name = file_name.or(file_path).unwrap_or_else(|| "Untitled".to_string());
        let stem = std::path::Path::new(&display_name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&display_name)
            .to_string();
        
        title_updates.push((id, if stem.is_empty() { "Untitled".to_string() } else { stem }));
    }
    drop(rows);
    drop(stmt);

    for (id, title) in title_updates {
        conn.execute("UPDATE notes SET title = ? WHERE note_id = ?", rusqlite::params![title, id])?;
    }

    // 2. Backfill content from file if empty and file_path is present
    let mut stmt = conn.prepare("SELECT note_id, file_path FROM notes WHERE content = '' OR content IS NULL")?;
    let mut rows = stmt.query([])?;
    let mut content_updates = Vec::new();
    while let Some(row) = rows.next()? {
        let id: i64 = row.get(0)?;
        let file_path: Option<String> = row.get(1)?;
        if let Some(path_str) = file_path {
            if !path_str.trim().is_empty() {
                if let Ok(file_content) = std::fs::read_to_string(&path_str) {
                    content_updates.push((id, file_content));
                }
            }
        }
    }
    drop(rows);
    drop(stmt);

    for (id, file_content) in content_updates {
        conn.execute("UPDATE notes SET content = ? WHERE note_id = ?", rusqlite::params![file_content, id])?;
    }

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
