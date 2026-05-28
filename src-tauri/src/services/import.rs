//! # Import Service
//!
//! Bulk import of Obsidian / plain markdown files into the Scribo note store.
//!
//! ## API
//!
//! - [`import_markdown_file`] — imports a single `.md` file, deriving the title from the filename.
//! - [`import_markdown_directory`] — recursively walks a directory and imports all `.md` files
//!   in a single SQLite transaction for atomicity and performance.
//!
//! ## Post-import
//!
//! Imported notes are inserted with `indexing_status = 'pending'`.
//! The [`scheduler`](crate::services::scheduler) will pick them up and index them asynchronously.

use std::path::Path;
use rusqlite::Connection;
use crate::error::AppError;
use crate::domain::note::NoteId;

/// Reads `path`, extracts the title from the filename (without extension), and inserts
/// a new note into the database. Returns the assigned [`NoteId`].
pub fn import_markdown_file(conn: &Connection, path: &Path) -> Result<NoteId, AppError> {
    let content = std::fs::read_to_string(path).map_err(|e| AppError::Other(e.to_string()))?;
    let title = path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Untitled")
        .to_string();

    let new_note = crate::domain::note::NewNote {
        title,
        content,
        ..Default::default()
    };

    crate::db::repos::notes::insert(conn, &new_note)
}

/// Recursively imports all `.md` files under `dir` within a single transaction.
/// Returns the number of files successfully imported.
///
/// The transaction is committed atomically — if any file fails to import,
/// the entire batch is rolled back.
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
