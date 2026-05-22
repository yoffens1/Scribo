use rusqlite::{Connection, Transaction};
use crate::error::AppError;

pub fn initialize_schema(conn: &mut Connection) -> Result<(), AppError> {
    // check_integrity MUST run first: if the DB is corrupt we want to fail fast
    // before doing any writes or migrations.
    check_integrity(conn)?;

    create_meta(conn)?;
    let version = get_schema_version(conn)?;
    create_files(conn)?;
    create_chunks(conn)?;
    create_cards(conn)?;
    create_history_tables(conn)?;
    apply_migrations(conn, version)?;

    // SAFETY: recover_interrupted must only be called during initial startup,
    // BEFORE any db operations. Otherwise it risks wiping active indexer work.
    recover_interrupted(conn)?;

    Ok(())
}

fn create_meta(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );"
    )?;
    Ok(())
}

fn get_schema_version(conn: &Connection) -> Result<i32, AppError> {
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

fn set_schema_version(conn: &Connection, version: i32) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO meta (key, value) VALUES ('schema_version', ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [version.to_string()],
    )?;
    Ok(())
}

fn create_files(conn: &Connection) -> Result<(), AppError> {
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
        );
        CREATE INDEX IF NOT EXISTS idx_files_path ON files(file_path);"
    )?;
    Ok(())
}

fn create_chunks(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS chunks (
            chunk_id INTEGER PRIMARY KEY AUTOINCREMENT,
            file_id INTEGER NOT NULL REFERENCES files(file_id) ON DELETE CASCADE,
            chunk_index INTEGER NOT NULL,
            chunk_text TEXT,
            token_count INTEGER,
            embedding BLOB NOT NULL,
            UNIQUE(file_id, chunk_index)
        );
        CREATE INDEX IF NOT EXISTS idx_chunks_file_id ON chunks(file_id);

        CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
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
        END;"
    )?;
    Ok(())
}

fn create_cards(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS cards (
            card_id INTEGER PRIMARY KEY AUTOINCREMENT,
            file_id INTEGER NOT NULL UNIQUE,
            anki_note_id INTEGER,
            state TEXT DEFAULT 'new',
            reps INTEGER DEFAULT 0,
            interval_days INTEGER DEFAULT 0,
            ease_factor REAL DEFAULT 2.5,
            next_review INTEGER,
            last_reviewed INTEGER,
            FOREIGN KEY (file_id) REFERENCES files(file_id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_cards_file_id ON cards(file_id);"
    )?;
    Ok(())
}

fn create_history_tables(conn: &Connection) -> Result<(), AppError> {
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

fn apply_migrations(conn: &mut Connection, mut from_version: i32) -> Result<(), AppError> {
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
    }

    tx.commit()?;
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

fn column_exists(conn: &Transaction, table: &str, column: &str) -> Result<bool, AppError> {
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

fn add_column_if_missing(
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
    // create_cards is idempotent (CREATE TABLE IF NOT EXISTS), safe to call again.
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS cards (
            card_id INTEGER PRIMARY KEY AUTOINCREMENT,
            file_id INTEGER NOT NULL UNIQUE,
            anki_note_id INTEGER,
            state TEXT DEFAULT 'new',
            reps INTEGER DEFAULT 0,
            interval_days INTEGER DEFAULT 0,
            ease_factor REAL DEFAULT 2.5,
            next_review INTEGER,
            last_reviewed INTEGER,
            FOREIGN KEY (file_id) REFERENCES files(file_id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_cards_file_id ON cards(file_id);"
    )?;
    Ok(())
}

fn migrate_v6(conn: &Transaction) -> Result<(), AppError> {
    // Add FTS5 virtual table and sync triggers for full-text search on chunk_text.
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
        END;"
    )?;

    // Backfill existing chunks into the FTS index.
    conn.execute_batch("INSERT INTO chunks_fts(chunks_fts) VALUES('rebuild');")?;

    Ok(())
}

fn recover_interrupted(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(
        "DELETE FROM chunks WHERE file_id IN (SELECT file_id FROM files WHERE status = 'indexing');
         UPDATE files SET status = 'failed', last_error = 'Interrupted indexing' WHERE status = 'indexing';"
    )?;
    Ok(())
}

fn check_integrity(conn: &Connection) -> Result<(), AppError> {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn open() -> Connection {
        Connection::open_in_memory().unwrap()
    }

    // Idempotency
    #[test]
    fn test_migrations_idempotency() {
        let mut conn = open();
        initialize_schema(&mut conn).expect("First initialization failed");
        initialize_schema(&mut conn).expect("Second initialization failed — not idempotent");
    }

    // column_exists helper
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

    // add_column_if_missing is idempotent
    #[test]
    fn test_add_column_if_missing_idempotent() {
        let mut conn = open();
        conn.execute_batch("CREATE TABLE t2 (id INTEGER PRIMARY KEY);").unwrap();
        let tx = conn.transaction().unwrap();
        add_column_if_missing(&tx, "t2", "extra", "INTEGER DEFAULT 0").unwrap();
        tx.commit().unwrap();
        // Second call must not error
        let tx = conn.transaction().unwrap();
        add_column_if_missing(&tx, "t2", "extra", "INTEGER DEFAULT 0").unwrap();
        tx.commit().unwrap();
        let tx = conn.transaction().unwrap();
        assert!(column_exists(&tx, "t2", "extra").unwrap());
        tx.rollback().unwrap();
    }

    // FTS INSERT trigger
    #[test]
    fn test_fts_search_after_init() {
        let mut conn = open();
        initialize_schema(&mut conn).expect("Init failed");
        conn.execute_batch(
            "INSERT INTO files (file_path, file_name) VALUES ('test.md', 'test.md');",
        ).unwrap();
        conn.execute(
            "INSERT INTO chunks (file_id, chunk_index, chunk_text, embedding) VALUES (1, 0, 'hello world', X'00')",
            [],
        ).unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM chunks_fts WHERE chunk_text MATCH 'hello'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count, 1, "INSERT trigger must index the chunk into FTS");
    }

    // migrate_v6 backfills pre-existing data
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
             INSERT INTO files (file_path, file_name) VALUES ('a.md', 'a.md');
             INSERT INTO chunks (file_id, chunk_index, chunk_text, embedding)
               VALUES (1, 0, 'existing chunk content', X'00');",
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

    // Full migration progression from v0
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
        for expected in &["cards", "chunks", "chunks_fts", "files", "meta"] {
            assert!(tables.iter().any(|t| t == expected), "Missing table: {}", expected);
        }
        for col in &["source_file_id", "embedding_model", "chunking_version", "status", "indexed_at"] {
            let tx = conn.unchecked_transaction().unwrap();
            assert!(column_exists(&tx, "files", col).unwrap(), "Missing column files.{}", col);
            tx.rollback().unwrap();
        }
        let version: i32 = conn.query_row(
            "SELECT CAST(value AS INTEGER) FROM meta WHERE key = 'schema_version'",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(version, 6, "Schema version must be 6 after full migration");
    }
}
