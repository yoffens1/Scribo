use rusqlite::Connection;
use crate::error::AppError;

pub fn create_meta(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );"
    )?;
    Ok(())
}

pub fn create_files(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS files (
            file_id INTEGER PRIMARY KEY AUTOINCREMENT,
            file_path TEXT NOT NULL UNIQUE,
            file_name TEXT NOT NULL,
            file_hash TEXT,
            file_mtime INTEGER,
            embedding_model TEXT DEFAULT 'unknown',
            embedding_dimension INTEGER,
            chunking_version TEXT DEFAULT '1',
            source_file_id INTEGER REFERENCES files(file_id) ON DELETE SET NULL,
            is_deleted INTEGER DEFAULT 0,
            status TEXT DEFAULT 'indexed',
            last_error TEXT,
            updated_at INTEGER,
            indexed_at INTEGER
        );"
    )?;
    Ok(())
}

pub fn create_chunks(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS chunks (
            chunk_id INTEGER PRIMARY KEY AUTOINCREMENT,
            file_id INTEGER NOT NULL REFERENCES files(file_id) ON DELETE CASCADE,
            chunk_index INTEGER NOT NULL,
            chunk_text TEXT,
            token_count INTEGER,
            embedding BLOB NOT NULL,
            UNIQUE(file_id, chunk_index)
        );"
    )?;
    Ok(())
}

pub fn create_cards(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS cards (
            card_id INTEGER PRIMARY KEY AUTOINCREMENT,
            file_id INTEGER NOT NULL UNIQUE REFERENCES files(file_id) ON DELETE CASCADE,
            anki_note_id INTEGER,
            state TEXT DEFAULT 'new',
            reps INTEGER DEFAULT 0,
            lapses INTEGER DEFAULT 0,
            stability REAL DEFAULT 0.0,
            difficulty REAL DEFAULT 0.0,
            next_review INTEGER,
            last_reviewed INTEGER
        );"
    )?;
    Ok(())
}

pub fn create_history_and_logs(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS files_history (
            history_id INTEGER PRIMARY KEY AUTOINCREMENT,
            file_id INTEGER NOT NULL REFERENCES files(file_id) ON DELETE CASCADE,
            patch TEXT NOT NULL,
            created_at INTEGER NOT NULL
         );
         CREATE TABLE IF NOT EXISTS review_logs (
            log_id INTEGER PRIMARY KEY AUTOINCREMENT,
            card_id INTEGER NOT NULL REFERENCES cards(card_id) ON DELETE CASCADE,
            rating INTEGER NOT NULL,
            reviewed_at INTEGER NOT NULL
         );"
    )?;
    Ok(())
}
