pub mod helpers;
pub mod tables;

use rusqlite::Connection;
use crate::error::AppError;

fn table_exists(conn: &Connection, name: &str) -> Result<bool, AppError> {
    let mut stmt = conn.prepare("SELECT 1 FROM sqlite_master WHERE type='table' AND name=?")?;
    let exists = stmt.exists([name])?;
    Ok(exists)
}

/// Главная функция инициализации схемы БД Scribo
pub fn initialize_schema(conn: &mut Connection) -> Result<(), AppError> {
    // 1. Проверяем целостность файла
    println!("Init: check_integrity");
    helpers::check_integrity(conn)?;

    let is_fresh = !table_exists(conn, "meta")?;

    if is_fresh {
        println!("Init: fresh database, creating all tables directly at v11");
        tables::create_schema(conn)?;
        conn.execute(
            "INSERT INTO meta (key, value) VALUES ('schema_version', '11')",
            [],
        )?;
        conn.execute(
            "INSERT INTO notes (title, path_cached, is_draft) VALUES ('_Inbox', '_Inbox', 0)",
            [],
        )?;
    } else {
        // Существующая БД — проверяем версию.
        let mut version: String = conn.query_row(
            "SELECT value FROM meta WHERE key = 'schema_version'",
            [],
            |r| r.get(0)
        )?;
        
        if version == "1" {
            println!("Init: upgrading database from v1 to v11");
            conn.execute_batch(
                "ALTER TABLE notes ADD COLUMN parent_note_id INTEGER REFERENCES notes(note_id) ON DELETE SET NULL;
                 ALTER TABLE notes ADD COLUMN path_cached TEXT NOT NULL DEFAULT '';
                 ALTER TABLE notes ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0;
                 ALTER TABLE notes ADD COLUMN icon TEXT;
                 ALTER TABLE notes ADD COLUMN is_draft INTEGER NOT NULL DEFAULT 0;
                 ALTER TABLE notes ADD COLUMN is_pinned INTEGER NOT NULL DEFAULT 0;
                 ALTER TABLE notes ADD COLUMN is_favorite INTEGER NOT NULL DEFAULT 0;
                 ALTER TABLE notes ADD COLUMN mastery REAL;
                 ALTER TABLE notes ADD COLUMN last_studied INTEGER;

                 CREATE INDEX IF NOT EXISTS idx_notes_parent ON notes(parent_note_id) WHERE is_deleted = 0;
                 CREATE INDEX IF NOT EXISTS idx_notes_path ON notes(path_cached);
                 CREATE INDEX IF NOT EXISTS idx_notes_drafts ON notes(updated_at DESC) WHERE is_draft = 1;
                 CREATE INDEX IF NOT EXISTS idx_notes_pinned ON notes(updated_at DESC) WHERE is_pinned = 1 AND is_deleted = 0;

                 INSERT INTO notes (title, path_cached, is_draft)
                 SELECT '_Inbox', '_Inbox', 0
                 WHERE NOT EXISTS (SELECT 1 FROM notes WHERE title = '_Inbox');

                 UPDATE meta SET value = '11' WHERE key = 'schema_version';"
            )?;
            version = "11".to_string();
        }
        
        if version != "11" {
            return Err(AppError::Other(format!(
                "Unsupported database version: got {}, expected 11", version
            )));
        }
    }

    // 2. Восстанавливаем прерванные задачи индексации
    println!("Init: recover_interrupted");
    helpers::recover_interrupted(conn)?;

    Ok(())
}
