use rusqlite::Connection;

pub fn initialize_schema(conn: &mut Connection) -> Result<(), String> {
    create_meta(conn)?;
    let version = get_schema_version(conn)?;
    create_files(conn)?;
    create_chunks(conn)?;
    create_cards(conn)?;
    apply_migrations(conn, version)?;
    recover_interrupted(conn)?;
    check_integrity(conn)?;
    Ok(())
}

fn create_meta(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );"
    ).map_err(|e| e.to_string())
}

fn get_schema_version(conn: &Connection) -> Result<i32, String> {
    let mut stmt = conn.prepare("SELECT value FROM meta WHERE key = 'schema_version'").map_err(|e| e.to_string())?;
    let mut rows = stmt.query([]).map_err(|e| e.to_string())?;
    
    if let Some(row) = rows.next().map_err(|e| e.to_string())? {
        let val: String = row.get(0).map_err(|e| e.to_string())?;
        val.parse::<i32>().map_err(|e| e.to_string())
    } else {
        Ok(0)
    }
}

fn set_schema_version(conn: &Connection, version: i32) -> Result<(), String> {
    conn.execute(
        "INSERT INTO meta (key, value) VALUES ('schema_version', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [version.to_string()],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

fn create_files(conn: &Connection) -> Result<(), String> {
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
    ).map_err(|e| e.to_string())
}

fn create_chunks(conn: &Connection) -> Result<(), String> {
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
        CREATE INDEX IF NOT EXISTS idx_chunks_file_id ON chunks(file_id);"
    ).map_err(|e| e.to_string())
}

fn create_cards(conn: &Connection) -> Result<(), String> {
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
    ).map_err(|e| e.to_string())
}

fn apply_migrations(conn: &mut Connection, mut from_version: i32) -> Result<(), String> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    if from_version < 1 {
        set_schema_version(&tx, 1)?;
        from_version = 1;
    }
    if from_version < 2 {
        migrate_v2(&tx)?;
        set_schema_version(&tx, 2)?;
    }
    if from_version < 3 {
        drop_metadata_column(&tx)?;
        set_schema_version(&tx, 3)?;
    }
    if from_version < 4 {
        add_performance_indexes(&tx)?;
        set_schema_version(&tx, 4)?;
    }
    if from_version < 5 {
        migrate_v5(&tx)?;
        set_schema_version(&tx, 5)?;
    }
    
    tx.commit().map_err(|e| e.to_string())
}

fn migrate_v2(conn: &rusqlite::Transaction) -> Result<(), String> {
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

    for (col, col_type) in additions {
        // Safe to ignore error if column already exists
        let _ = conn.execute_batch(&format!("ALTER TABLE files ADD COLUMN {} {};", col, col_type));
    }
    let _ = conn.execute_batch("ALTER TABLE chunks ADD COLUMN token_count INTEGER;");
    Ok(())
}

fn drop_metadata_column(conn: &rusqlite::Transaction) -> Result<(), String> {
    // Check if column exists before dropping
    let mut stmt = conn.prepare("PRAGMA table_info('chunks')").map_err(|e| e.to_string())?;
    let mut rows = stmt.query([]).map_err(|e| e.to_string())?;
    
    let mut has_metadata = false;
    while let Some(row) = rows.next().map_err(|e| e.to_string())? {
        let name: String = row.get(1).map_err(|e| e.to_string())?;
        if name == "metadata" {
            has_metadata = true;
            break;
        }
    }

    if has_metadata {
        let _ = conn.execute_batch("ALTER TABLE chunks DROP COLUMN metadata");
    }
    Ok(())
}

fn add_performance_indexes(conn: &rusqlite::Transaction) -> Result<(), String> {
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_files_deleted_status ON files(is_deleted, status);
         CREATE INDEX IF NOT EXISTS idx_files_file_name ON files(file_name);"
    ).map_err(|e| e.to_string())
}

fn migrate_v5(conn: &rusqlite::Transaction) -> Result<(), String> {
    let _ = conn.execute_batch("ALTER TABLE files ADD COLUMN source_file_id INTEGER REFERENCES files(file_id) ON DELETE SET NULL;");
    let _ = create_cards(conn);
    Ok(())
}

fn recover_interrupted(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "DELETE FROM chunks WHERE file_id IN (SELECT file_id FROM files WHERE status = 'indexing');
         UPDATE files SET status = 'failed', last_error = 'Interrupted indexing' WHERE status = 'indexing';"
    ).map_err(|e| e.to_string())
}

fn check_integrity(conn: &Connection) -> Result<(), String> {
    let mut stmt = conn.prepare("PRAGMA integrity_check;").map_err(|e| e.to_string())?;
    let mut rows = stmt.query([]).map_err(|e| e.to_string())?;
    
    if let Some(row) = rows.next().map_err(|e| e.to_string())? {
        let val: String = row.get(0).map_err(|e| e.to_string())?;
        if val != "ok" {
            return Err("Database corruption detected! Integrity check failed.".to_string());
        }
    }
    Ok(())
}
