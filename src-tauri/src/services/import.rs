use std::path::Path;
use rusqlite::{params, Connection};
use crate::error::AppError;
use crate::domain::note::NoteId;

pub fn import_markdown_file(conn: &Connection, path: &Path) -> Result<NoteId, AppError> {
    let content = std::fs::read_to_string(path).map_err(|e| AppError::Other(e.to_string()))?;
    let title = path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Untitled")
        .to_string();

    let content_hash = blake3::hash(content.as_bytes()).to_hex().to_string();
    let now = crate::db::time::now_seconds();

    let note_id: i64 = conn.query_row(
        "INSERT INTO notes (title, content, content_hash, indexing_status, created_at, updated_at)
         VALUES (?, ?, ?, 'pending', ?, ?)
         RETURNING note_id",
        params![title, content, content_hash, now, now],
        |row| row.get(0),
    )?;

    Ok(NoteId(note_id))
}

pub fn import_markdown_directory(conn: &mut Connection, dir: &Path) -> Result<usize, AppError> {
    let tx = conn.transaction().map_err(|e| AppError::Other(e.to_string()))?;
    let mut count = 0;
    for entry in walkdir::WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        if entry.path().extension().and_then(|s| s.to_str()) == Some("md") {
            import_markdown_file(&tx, entry.path())?;
            count += 1;
        }
    }
    tx.commit().map_err(|e| AppError::Other(e.to_string()))?;
    Ok(count)
}
