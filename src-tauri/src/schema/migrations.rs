use rusqlite::{Connection, Transaction};
use crate::error::AppError;
use crate::schema::helpers::add_column_if_missing;

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
    use crate::schema::helpers::column_exists;
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
    use crate::schema::helpers::column_exists;
    
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::initialize_schema;
    use crate::schema::helpers::column_exists;

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
        assert_eq!(version, 8, "Schema version must be 8 after full migration");
    }
}
