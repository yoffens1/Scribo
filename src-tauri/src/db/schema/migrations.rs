use rusqlite::{Connection, Transaction};
use crate::error::AppError;
use crate::db::schema::helpers::add_column_if_missing;

pub fn get_schema_version(conn: &Connection) -> Result<i32, AppError> {
    let mut stmt = conn.prepare("SELECT value FROM meta WHERE key = 'schema_version'")?;
    let mut rows = stmt.query([])?;

    if let Some(row) = rows.next()? {
        let val: String = row.get(0)?;
        val.parse::<i32>()
            .map_err(|e| AppError::Other(format!("Invalid schema_version: {}", e)))
    } else {
        Ok(0)
    }
}

pub fn set_schema_version(conn: &Connection, version: i32) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO meta (key, value) VALUES ('schema_version', ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [version.to_string()],
    )?;
    Ok(())
}

pub fn apply_migrations(conn: &mut Connection, mut from_version: i32) -> Result<(), AppError> {
    let tx = conn.transaction()?;

    if from_version < 1 {
        set_schema_version(&tx, 1)?;
        from_version = 1;
    }
    if from_version < 2 {
        migrate_v2(&tx)?;
        set_schema_version(&tx, 2)?;
        from_version = 2;
    }
    if from_version < 3 {
        drop_metadata_column(&tx)?;
        set_schema_version(&tx, 3)?;
        from_version = 3;
    }
    if from_version < 4 {
        add_performance_indexes(&tx)?;
        set_schema_version(&tx, 4)?;
        from_version = 4;
    }
    if from_version < 5 {
        migrate_v5(&tx)?;
        set_schema_version(&tx, 5)?;
        from_version = 5;
    }
    if from_version < 6 {
        migrate_v6(&tx)?;
        set_schema_version(&tx, 6)?;
        from_version = 6;
    }
    if from_version < 7 {
        migrate_v7(&tx)?;
        set_schema_version(&tx, 7)?;
        from_version = 7;
    }
    if from_version < 8 {
        migrate_v8(&tx)?;
        set_schema_version(&tx, 8)?;
        from_version = 8;
    }
    if from_version < 9 {
        migrate_v9(&tx)?;
        set_schema_version(&tx, 9)?;
        from_version = 9;
    }
    if from_version < 10 {
        migrate_v10(&tx)?;
        set_schema_version(&tx, 10)?;
        from_version = 10;
    }
    if from_version < 11 {
        migrate_v11(&tx)?;
        set_schema_version(&tx, 11)?;
    }

    tx.commit()?;
    Ok(())
}

fn migrate_v2(conn: &Transaction) -> Result<(), AppError> {
    let additions = vec![
        ("is_deleted", "INTEGER DEFAULT 0"),
        ("embedding_model", "TEXT DEFAULT 'unknown'"),
        ("chunking_version", "TEXT DEFAULT '1'"),
        ("embedding_dimension", "INTEGER"),
        ("file_mtime", "INTEGER"),
        ("status", "TEXT DEFAULT 'indexed'"),
        ("last_error", "TEXT"),
        ("updated_at", "INTEGER"),
        ("indexed_at", "INTEGER"),
    ];
    for (col, def) in additions {
        add_column_if_missing(conn, "files", col, def)?;
    }
    add_column_if_missing(conn, "chunks", "token_count", "INTEGER")?;
    Ok(())
}

fn drop_metadata_column(conn: &Transaction) -> Result<(), AppError> {
    use crate::db::schema::helpers::column_exists;
    if column_exists(conn, "chunks", "metadata")? {
        conn.execute_batch("ALTER TABLE chunks DROP COLUMN metadata;")?;
    }
    Ok(())
}

fn add_performance_indexes(conn: &Transaction) -> Result<(), AppError> {
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_files_deleted_status ON files(is_deleted, status);
         CREATE INDEX IF NOT EXISTS idx_files_file_name ON files(file_name);"
    )?;
    Ok(())
}

fn migrate_v5(conn: &Transaction) -> Result<(), AppError> {
    add_column_if_missing(
        conn,
        "files",
        "source_file_id",
        "INTEGER REFERENCES files(file_id) ON DELETE SET NULL",
    )?;
    Ok(())
}

fn migrate_v6(conn: &Transaction) -> Result<(), AppError> {
    conn.execute_batch(
        "CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
            chunk_text,
            content='chunks',
            content_rowid='chunk_id',
            tokenize='unicode61 remove_diacritics 1'
        );

        CREATE TRIGGER IF NOT EXISTS chunks_fts_insert AFTER INSERT ON chunks BEGIN
          INSERT INTO chunks_fts(rowid, chunk_text) VALUES (new.chunk_id, new.chunk_text);
        END;

        CREATE TRIGGER IF NOT EXISTS chunks_fts_delete AFTER DELETE ON chunks BEGIN
          INSERT INTO chunks_fts(chunks_fts, rowid, chunk_text) VALUES('delete', old.chunk_id, old.chunk_text);
        END;

        CREATE TRIGGER IF NOT EXISTS chunks_fts_update AFTER UPDATE ON chunks BEGIN
          INSERT INTO chunks_fts(chunks_fts, rowid, chunk_text) VALUES('delete', old.chunk_id, old.chunk_text);
          INSERT INTO chunks_fts(rowid, chunk_text) VALUES (new.chunk_id, new.chunk_text);
        END;
        
        INSERT INTO chunks_fts(chunks_fts) VALUES('rebuild');"
    )?;
    Ok(())
}

fn migrate_v7(conn: &Transaction) -> Result<(), AppError> {
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

fn migrate_v8(conn: &Transaction) -> Result<(), AppError> {
    use crate::db::schema::helpers::column_exists;
    
    // Rename SM-2 columns to FSRS terminology and add difficulty
    if column_exists(conn, "cards", "interval_days")? {
        conn.execute_batch(
            "ALTER TABLE cards RENAME COLUMN interval_days TO lapses;
             ALTER TABLE cards RENAME COLUMN ease_factor TO stability;
             ALTER TABLE cards ADD COLUMN difficulty REAL DEFAULT 0.0;"
        )?;
    } else if !column_exists(conn, "cards", "difficulty")? {
        // Fallback if somehow created without interval_days but lacking difficulty
        conn.execute_batch("ALTER TABLE cards ADD COLUMN difficulty REAL DEFAULT 0.0;")?;
    }

    // Drop redundant indexes (file_path is UNIQUE, file_id is UNIQUE, etc.)
    conn.execute_batch(
        "DROP INDEX IF EXISTS idx_files_path;
         DROP INDEX IF EXISTS idx_chunks_file_id;
         DROP INDEX IF EXISTS idx_cards_file_id;"
    )?;
    Ok(())
}

fn migrate_v9(conn: &Transaction) -> Result<(), AppError> {
    let sql = include_str!("migrations/v9_up.sql");
    conn.execute_batch(sql)?;
    Ok(())
}

fn migrate_v10(conn: &Transaction) -> Result<(), AppError> {
    let sql = include_str!("migrations/v10_up.sql");
    conn.execute_batch(sql)?;
    Ok(())
}

fn migrate_v11(conn: &Transaction) -> Result<(), AppError> {
    let sql = include_str!("migrations/v11_up.sql");
    conn.execute_batch(sql)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema::initialize_schema;
    use crate::db::schema::helpers::column_exists;

    fn open() -> Connection {
        Connection::open_in_memory().unwrap()
    }

    #[test]
    fn test_migrations_idempotency() {
        let mut conn = open();
        initialize_schema(&mut conn).expect("First initialization failed");
        initialize_schema(&mut conn).expect("Second initialization failed — not idempotent");
    }

    #[test]
    fn test_column_exists() {
        let conn = open();
        conn.execute_batch("CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT);").unwrap();
        let tx = conn.unchecked_transaction().unwrap();
        assert!(column_exists(&tx, "t", "id").unwrap());
        assert!(column_exists(&tx, "t", "name").unwrap());
        assert!(!column_exists(&tx, "t", "nonexistent").unwrap());
        tx.rollback().unwrap();
    }

    #[test]
    fn test_add_column_if_missing_idempotent() {
        let mut conn = open();
        conn.execute_batch("CREATE TABLE t2 (id INTEGER PRIMARY KEY);").unwrap();
        let tx = conn.transaction().unwrap();
        add_column_if_missing(&tx, "t2", "extra", "INTEGER DEFAULT 0").unwrap();
        tx.commit().unwrap();
        
        let tx = conn.transaction().unwrap();
        add_column_if_missing(&tx, "t2", "extra", "INTEGER DEFAULT 0").unwrap();
        tx.commit().unwrap();
        
        let tx = conn.transaction().unwrap();
        assert!(column_exists(&tx, "t2", "extra").unwrap());
        tx.rollback().unwrap();
    }

    #[test]
    fn test_fts_search_after_init() {
        let mut conn = open();
        initialize_schema(&mut conn).expect("Init failed");
        conn.execute_batch(
            "INSERT INTO notes (file_path, file_name, title, content) VALUES ('test.md', 'test.md', 'test', 'test');",
        ).unwrap();
        conn.execute(
            "INSERT INTO fragments (note_id, fragment_index, text, embedding) VALUES (1, 0, 'hello world', X'00')",
            [],
        ).unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM fragments_fts WHERE text MATCH 'hello'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count, 1, "INSERT trigger must index the fragment into FTS");
    }

    #[test]
    fn test_fts_backfill_on_migrate_v6() {
        let mut conn = open();
        conn.execute_batch(
            "CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             INSERT INTO meta VALUES ('schema_version', '5');
             CREATE TABLE files (
                 file_id INTEGER PRIMARY KEY AUTOINCREMENT,
                 file_path TEXT NOT NULL UNIQUE,
                 file_name TEXT NOT NULL,
                 is_deleted INTEGER DEFAULT 0
             );
             CREATE TABLE chunks (
                 chunk_id INTEGER PRIMARY KEY AUTOINCREMENT,
                 file_id INTEGER NOT NULL,
                 chunk_index INTEGER NOT NULL,
                 chunk_text TEXT,
                 token_count INTEGER,
                 embedding BLOB NOT NULL,
                 UNIQUE(file_id, chunk_index)
             );
             INSERT INTO chunks (file_id, chunk_index, chunk_text, embedding)
               VALUES (1, 0, 'existing fragment content', X'00');",
        ).unwrap();
        let tx = conn.transaction().unwrap();
        migrate_v6(&tx).expect("migrate_v6 failed");
        tx.commit().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM chunks_fts WHERE chunk_text MATCH 'existing'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count, 1, "migrate_v6 must backfill pre-existing chunks into FTS");
    }

    #[test]
    fn test_migration_progression_from_v0() {
        let mut conn = open();
        initialize_schema(&mut conn).expect("Fresh init failed");
        let tables: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
                .unwrap();
            stmt.query_map([], |r| r.get(0))
                .unwrap()
                .collect::<rusqlite::Result<_>>()
                .unwrap()
        };
        for expected in &["cards", "fragments", "fragments_fts", "notes", "meta", "schedules", "note_revisions", "review_logs"] {
            assert!(tables.iter().any(|t| t == expected), "Missing table: {}", expected);
        }
        for col in &["source_note_id", "embedding_model", "indexing_version", "indexing_status", "indexed_at", "title", "content", "tags"] {
            let tx = conn.unchecked_transaction().unwrap();
            assert!(column_exists(&tx, "notes", col).unwrap(), "Missing column notes.{}", col);
            tx.rollback().unwrap();
        }
        let version: i32 = conn.query_row(
            "SELECT CAST(value AS INTEGER) FROM meta WHERE key = 'schema_version'",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(version, 11, "Schema version must be 11 after full migration");
    }

    #[test]
    fn test_backfill_after_migration() {
        let mut conn = open();
        // Set up pre-v10 state manually
        conn.execute_batch(
            "CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             INSERT INTO meta VALUES ('schema_version', '9');
             CREATE TABLE files (
                 file_id INTEGER PRIMARY KEY AUTOINCREMENT,
                 file_path TEXT NOT NULL UNIQUE,
                 file_name TEXT NOT NULL,
                 status TEXT,
                 last_error TEXT,
                 source_file_id INTEGER,
                 chunking_version TEXT
             );
             CREATE TABLE chunks (
                 chunk_id INTEGER PRIMARY KEY AUTOINCREMENT,
                 file_id INTEGER NOT NULL,
                 chunk_index INTEGER NOT NULL,
                 chunk_text TEXT,
                 embedding BLOB NOT NULL
             );
             CREATE TABLE cards (
                 card_id INTEGER PRIMARY KEY AUTOINCREMENT,
                 file_id INTEGER NOT NULL UNIQUE REFERENCES files(file_id) ON DELETE CASCADE,
                 anki_note_id INTEGER,
                 front TEXT,
                 back TEXT,
                 card_type TEXT,
                 source_fragment_id INTEGER,
                 source_offset INTEGER,
                 source_length INTEGER,
                 generated_by TEXT,
                 is_suspended INTEGER
             );
             CREATE TABLE schedules (
                 schedule_id INTEGER PRIMARY KEY AUTOINCREMENT,
                 target_type TEXT NOT NULL,
                 target_id INTEGER NOT NULL
             );
             CREATE TABLE files_history (
                 history_id INTEGER PRIMARY KEY AUTOINCREMENT,
                 file_id INTEGER NOT NULL,
                 patch TEXT NOT NULL,
                 created_at INTEGER NOT NULL
             );
             INSERT INTO files (file_path, file_name) VALUES ('/path/to/my_file.md', 'my_file.md');"
        ).unwrap();

        // Write a temp file matching the path so we can test content reading
        let temp_dir = std::env::temp_dir();
        let test_file_path = temp_dir.join("test_scribo_note.md");
        std::fs::write(&test_file_path, "Hello from filesystem!").unwrap();
        let path_str = test_file_path.to_string_lossy().to_string();

        conn.execute(
            "INSERT INTO files (file_path, file_name) VALUES (?, ?);",
            rusqlite::params![path_str, "test_scribo_note.md"],
        ).unwrap();

        // Apply migration v10
        let tx = conn.transaction().unwrap();
        migrate_v10(&tx).unwrap();
        tx.commit().unwrap();

        // Perform backfill
        crate::db::schema::helpers::backfill_notes_after_migration(&conn).unwrap();

        // Check if title is backfilled
        let title1: String = conn.query_row("SELECT title FROM notes WHERE file_name = 'my_file.md'", [], |r| r.get(0)).unwrap();
        assert_eq!(title1, "my_file");

        let title2: String = conn.query_row("SELECT title FROM notes WHERE file_name = 'test_scribo_note.md'", [], |r| r.get(0)).unwrap();
        assert_eq!(title2, "test_scribo_note");

        // Check if content is backfilled from temp file
        let content2: String = conn.query_row("SELECT content FROM notes WHERE file_name = 'test_scribo_note.md'", [], |r| r.get(0)).unwrap();
        assert_eq!(content2, "Hello from filesystem!");

        // Clean up temp file
        let _ = std::fs::remove_file(test_file_path);
    }

    #[test]
    fn test_v9_preserves_fsrs_state_in_schedules() {
        let mut conn = open();
        conn.execute_batch(
            "CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             INSERT INTO meta VALUES ('schema_version', '8');
             CREATE TABLE files (
                 file_id INTEGER PRIMARY KEY AUTOINCREMENT,
                 file_path TEXT NOT NULL UNIQUE,
                 file_name TEXT NOT NULL
             );
             CREATE TABLE cards (
                 card_id INTEGER PRIMARY KEY AUTOINCREMENT,
                 file_id INTEGER NOT NULL UNIQUE REFERENCES files(file_id) ON DELETE CASCADE,
                 state TEXT,
                 stability REAL,
                 difficulty REAL,
                 reps INTEGER,
                 lapses INTEGER,
                 last_reviewed INTEGER,
                 next_review INTEGER
             );
             CREATE TABLE review_logs (
                 log_id INTEGER PRIMARY KEY AUTOINCREMENT,
                 card_id INTEGER NOT NULL REFERENCES cards(card_id) ON DELETE CASCADE,
                 rating INTEGER NOT NULL,
                 reviewed_at INTEGER NOT NULL
             );
             INSERT INTO files (file_path, file_name) VALUES ('test.md', 'test.md');
             INSERT INTO cards (file_id, state, stability, difficulty, reps, lapses, last_reviewed, next_review) 
               VALUES (1, 'review', 5.5, 4.4, 3, 1, 1000, 2000);"
        ).unwrap();

        let tx = conn.transaction().unwrap();
        migrate_v9(&tx).unwrap();
        tx.commit().unwrap();

        let row = conn.query_row(
            "SELECT state, stability, difficulty, reps, lapses, last_reviewed, next_review 
             FROM schedules WHERE target_type = 'card' AND target_id = 1",
            [],
            |r| Ok((
                r.get::<_, String>(0)?,
                r.get::<_, f64>(1)?,
                r.get::<_, f64>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, i64>(5)?,
                r.get::<_, i64>(6)?,
            ))
        ).unwrap();

        assert_eq!(row.0, "review");
        assert_eq!(row.1, 5.5);
        assert_eq!(row.2, 4.4);
        assert_eq!(row.3, 3);
        assert_eq!(row.4, 1);
        assert_eq!(row.5, 1000);
        assert_eq!(row.6, 2000);

        // Verify card FSRS fields are gone
        let tx = conn.unchecked_transaction().unwrap();
        assert!(!column_exists(&tx, "cards", "stability").unwrap());
    }

    #[test]
    fn test_v10_renames_preserve_data() {
        let mut conn = open();
        conn.execute_batch(
            "CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             INSERT INTO meta VALUES ('schema_version', '9');
             CREATE TABLE files (
                 file_id INTEGER PRIMARY KEY AUTOINCREMENT,
                 file_path TEXT NOT NULL UNIQUE,
                 file_name TEXT NOT NULL,
                 status TEXT,
                 last_error TEXT,
                 source_file_id INTEGER,
                 chunking_version TEXT
             );
             CREATE TABLE chunks (
                 chunk_id INTEGER PRIMARY KEY AUTOINCREMENT,
                 file_id INTEGER NOT NULL,
                 chunk_index INTEGER NOT NULL,
                 chunk_text TEXT,
                 embedding BLOB NOT NULL
             );
             CREATE TABLE cards (
                 card_id INTEGER PRIMARY KEY AUTOINCREMENT,
                 file_id INTEGER NOT NULL UNIQUE REFERENCES files(file_id) ON DELETE CASCADE,
                 anki_note_id INTEGER,
                 front TEXT,
                 back TEXT,
                 card_type TEXT NOT NULL DEFAULT 'basic',
                 source_fragment_id INTEGER,
                 source_offset INTEGER,
                 source_length INTEGER,
                 generated_by TEXT,
                 is_suspended INTEGER NOT NULL DEFAULT 0
             );
             CREATE TABLE schedules (
                 schedule_id INTEGER PRIMARY KEY AUTOINCREMENT,
                 target_type TEXT NOT NULL,
                 target_id INTEGER NOT NULL
             );
             CREATE TABLE files_history (
                 history_id INTEGER PRIMARY KEY AUTOINCREMENT,
                 file_id INTEGER NOT NULL,
                 patch TEXT NOT NULL,
                 created_at INTEGER NOT NULL
             );
             INSERT INTO files (file_id, file_path, file_name, status, chunking_version) 
               VALUES (42, 'data.md', 'data.md', 'indexed', 'v1');
             INSERT INTO chunks (file_id, chunk_index, chunk_text, embedding) 
               VALUES (42, 0, 'some searchable data', X'00');
             INSERT INTO cards (file_id, front, back) VALUES (42, 'q', 'a');"
        ).unwrap();

        let tx = conn.transaction().unwrap();
        migrate_v10(&tx).unwrap();
        tx.commit().unwrap();

        // 1. Verify files -> notes renamed correctly
        let note_id: i64 = conn.query_row("SELECT note_id FROM notes WHERE file_path = 'data.md'", [], |r| r.get(0)).unwrap();
        assert_eq!(note_id, 42);
        
        let indexing_status: String = conn.query_row("SELECT indexing_status FROM notes WHERE note_id = 42", [], |r| r.get(0)).unwrap();
        assert_eq!(indexing_status, "indexed");
        
        let fragmenting_ver: String = conn.query_row("SELECT fragmenting_version FROM notes WHERE note_id = 42", [], |r| r.get(0)).unwrap();
        assert_eq!(fragmenting_ver, "v1");

        // 2. Verify chunks -> fragments
        let text: String = conn.query_row("SELECT text FROM fragments WHERE note_id = 42", [], |r| r.get(0)).unwrap();
        assert_eq!(text, "some searchable data");

        // 3. Verify fragments_fts works
        let fts_count: i64 = conn.query_row("SELECT COUNT(*) FROM fragments_fts WHERE text MATCH 'searchable'", [], |r| r.get(0)).unwrap();
        assert_eq!(fts_count, 1);
    }
}
