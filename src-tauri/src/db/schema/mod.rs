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
        println!("Init: fresh database, creating all tables directly at v15");
        tables::create_schema(conn)?;
        conn.execute(
            "INSERT INTO meta (key, value) VALUES ('schema_version', '15')",
            [],
        )?;
        conn.execute(
            "INSERT INTO notes (title, path_cached, lifecycle) VALUES ('_Inbox', '_Inbox', 'active')",
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

        if version == "11" {
            println!("Init: upgrading database from v11 to v12");
            conn.execute_batch(
                "ALTER TABLE sections ADD COLUMN content_offset_start INTEGER NOT NULL DEFAULT 0;
                 ALTER TABLE sections ADD COLUMN content_offset_end INTEGER NOT NULL DEFAULT 0;

                 CREATE TABLE IF NOT EXISTS distribution_runs (
                    run_id              INTEGER PRIMARY KEY AUTOINCREMENT,
                    draft_id            INTEGER NOT NULL,
                    plan_json           TEXT NOT NULL,
                    result_json         TEXT,
                    generator_version   TEXT NOT NULL,
                    status              TEXT NOT NULL CHECK (status IN ('analyzed', 'applied', 'cancelled')),
                    created_at          INTEGER NOT NULL DEFAULT (strftime('%s','now')),
                    applied_at          INTEGER
                 );

                 UPDATE meta SET value = '12' WHERE key = 'schema_version';"
            )?;
            version = "12".to_string();
        }

        if version == "12" {
            println!("Init: upgrading database from v12 to v13 (unified chunks)");
            
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS chunks (
                    chunk_id                INTEGER PRIMARY KEY AUTOINCREMENT,
                    note_id                 INTEGER NOT NULL REFERENCES notes(note_id) ON DELETE CASCADE,
                    parent_chunk_id         INTEGER REFERENCES chunks(chunk_id) ON DELETE CASCADE,
                    level                   INTEGER NOT NULL,
                    order_index             INTEGER NOT NULL,
                    raw_text                TEXT NOT NULL,
                    raw_text_hash           TEXT NOT NULL,
                    clean_text              TEXT NOT NULL,
                    clean_text_hash         TEXT NOT NULL,
                    embedding               BLOB,
                    embedding_source        TEXT,
                    embedding_model         TEXT,
                    embedding_model_version TEXT,
                    embedded_at             INTEGER,
                    heading                 TEXT,
                    heading_level           INTEGER,
                    content_offset_start    INTEGER NOT NULL DEFAULT 0,
                    content_offset_end      INTEGER NOT NULL DEFAULT 0,
                    token_count             INTEGER,
                    kind                    TEXT NOT NULL DEFAULT 'fragment',
                    deleted_at              INTEGER,
                    created_at              INTEGER NOT NULL DEFAULT (strftime('%s','now')),
                    updated_at              INTEGER NOT NULL DEFAULT (strftime('%s','now'))
                 );

                 CREATE TABLE IF NOT EXISTS embedding_cache (
                    clean_text_hash         TEXT NOT NULL,
                    embedding_model         TEXT NOT NULL,
                    embedding_model_version TEXT NOT NULL,
                    embedding               BLOB NOT NULL,
                    created_at              INTEGER NOT NULL,
                    PRIMARY KEY (clean_text_hash, embedding_model, embedding_model_version)
                 );
                 
                 CREATE INDEX IF NOT EXISTS idx_chunks_note_level ON chunks(note_id, level);
                 CREATE INDEX IF NOT EXISTS idx_chunks_parent ON chunks(parent_chunk_id);
                 CREATE INDEX IF NOT EXISTS idx_chunks_clean_hash ON chunks(clean_text_hash);
                 CREATE INDEX IF NOT EXISTS idx_chunks_embedded_alive ON chunks(level) WHERE deleted_at IS NULL AND embedding IS NOT NULL;
                "
            )?;

            conn.execute_batch("PRAGMA foreign_keys = OFF;")?;

            conn.execute_batch(
                "INSERT INTO chunks (chunk_id, note_id, level, order_index, raw_text, raw_text_hash, clean_text, clean_text_hash, heading, heading_level, content_offset_start, content_offset_end, kind, created_at, updated_at)
                 SELECT section_id, note_id, 0, section_index, text_raw, source_hash, text_raw, source_hash, heading, heading_level, content_offset_start, content_offset_end, 'heading_block', created_at, created_at FROM sections;"
            )?;

            conn.execute_batch(
                "INSERT INTO chunks (note_id, parent_chunk_id, level, order_index, raw_text, raw_text_hash, clean_text, clean_text_hash, embedding, token_count, kind, created_at, updated_at)
                 SELECT f.note_id, c.chunk_id, 1, f.fragment_index, f.text_clean, f.source_hash, f.text_clean, f.source_hash, f.embedding, f.token_count, 'fragment', strftime('%s','now'), strftime('%s','now')
                 FROM fragments f
                 LEFT JOIN chunks c ON c.note_id = f.note_id AND c.level = 0 AND c.order_index = 0;"
            )?;

            conn.execute_batch(
                "CREATE TABLE cards_new (
                    card_id INTEGER PRIMARY KEY AUTOINCREMENT,
                    chunk_id INTEGER NOT NULL REFERENCES chunks(chunk_id) ON DELETE CASCADE,
                    card_type TEXT NOT NULL DEFAULT 'heading' CHECK (card_type IN ('heading', 'qa', 'cloze', 'manual')),
                    custom_front TEXT,
                    custom_back TEXT,
                    cloze_mask TEXT,
                    is_stale INTEGER NOT NULL DEFAULT 0,
                    is_suspended INTEGER NOT NULL DEFAULT 0,
                    generated_by TEXT,
                    section_hash_at_creation TEXT,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                 );

                 INSERT INTO cards_new (card_id, chunk_id, card_type, custom_front, custom_back, cloze_mask, is_stale, is_suspended, generated_by, section_hash_at_creation, created_at, updated_at)
                 SELECT card_id, section_id, card_type, custom_front, custom_back, cloze_mask, is_stale, is_suspended, generated_by, section_hash_at_creation, created_at, updated_at FROM cards;

                 DROP TABLE cards;
                 ALTER TABLE cards_new RENAME TO cards;
                 CREATE INDEX IF NOT EXISTS idx_cards_section_id ON cards(chunk_id);
                 CREATE INDEX IF NOT EXISTS idx_cards_stale ON cards(chunk_id) WHERE is_stale = 1;
                 CREATE INDEX IF NOT EXISTS idx_cards_not_suspended ON cards(chunk_id) WHERE is_suspended = 0;
                "
            )?;

            conn.execute_batch(
                "DROP TABLE IF EXISTS fragments_fts;
                 CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
                     clean_text,
                     content='chunks',
                     content_rowid='chunk_id',
                     tokenize = 'unicode61 remove_diacritics 2'
                 );

                 CREATE TRIGGER IF NOT EXISTS chunks_fts_insert AFTER INSERT ON chunks WHEN NEW.level = 1 BEGIN
                     INSERT INTO chunks_fts(rowid, clean_text) VALUES (NEW.chunk_id, NEW.clean_text);
                 END;

                 CREATE TRIGGER IF NOT EXISTS chunks_fts_delete AFTER DELETE ON chunks WHEN OLD.level = 1 BEGIN
                     INSERT INTO chunks_fts(chunks_fts, rowid, clean_text) VALUES('delete', OLD.chunk_id, OLD.clean_text);
                 END;

                 CREATE TRIGGER IF NOT EXISTS chunks_fts_update AFTER UPDATE OF clean_text ON chunks WHEN NEW.level = 1 BEGIN
                     INSERT INTO chunks_fts(chunks_fts, rowid, clean_text) VALUES('delete', OLD.chunk_id, OLD.clean_text);
                     INSERT INTO chunks_fts(rowid, clean_text) VALUES (NEW.chunk_id, NEW.clean_text);
                 END;

                 INSERT INTO chunks_fts(rowid, clean_text)
                 SELECT chunk_id, clean_text FROM chunks WHERE level = 1;
                "
            )?;

            conn.execute_batch(
                "DROP TABLE fragments;
                 DROP TABLE sections;
                 UPDATE meta SET value = '13' WHERE key = 'schema_version';"
            )?;

            conn.execute_batch("PRAGMA foreign_keys = ON;")?;
            version = "13".to_string();
        }

        if version == "13" {
            println!("Init: upgrading database from v13 to v14 (hierarchical tags)");
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS tags (
                    tag_id          INTEGER PRIMARY KEY AUTOINCREMENT,
                    parent_tag_id   INTEGER REFERENCES tags(tag_id) ON DELETE CASCADE,
                    name            TEXT NOT NULL,
                    slug            TEXT NOT NULL,
                    color           TEXT,
                    icon            TEXT,
                    depth           INTEGER NOT NULL DEFAULT 0,
                    path_cached     TEXT NOT NULL,
                    description     TEXT,
                    created_at      INTEGER NOT NULL,
                    updated_at      INTEGER NOT NULL,
                    CHECK (parent_tag_id IS NULL OR parent_tag_id <> tag_id)
                );

                CREATE UNIQUE INDEX IF NOT EXISTS idx_tags_root_slug ON tags(slug) WHERE parent_tag_id IS NULL;
                CREATE UNIQUE INDEX IF NOT EXISTS idx_tags_child_slug ON tags(parent_tag_id, slug) WHERE parent_tag_id IS NOT NULL;
                CREATE INDEX IF NOT EXISTS idx_tags_parent ON tags(parent_tag_id);
                CREATE INDEX IF NOT EXISTS idx_tags_slug ON tags(slug);
                CREATE INDEX IF NOT EXISTS idx_tags_path ON tags(path_cached);

                CREATE TABLE IF NOT EXISTS tag_closure (
                    ancestor_id   INTEGER NOT NULL REFERENCES tags(tag_id) ON DELETE CASCADE,
                    descendant_id INTEGER NOT NULL REFERENCES tags(tag_id) ON DELETE CASCADE,
                    depth         INTEGER NOT NULL,
                    PRIMARY KEY (ancestor_id, descendant_id)
                );

                CREATE INDEX IF NOT EXISTS idx_tag_closure_desc ON tag_closure(descendant_id);

                CREATE TABLE IF NOT EXISTS note_tags (
                    note_id     INTEGER NOT NULL REFERENCES notes(note_id) ON DELETE CASCADE,
                    tag_id      INTEGER NOT NULL REFERENCES tags(tag_id) ON DELETE CASCADE,
                    source      TEXT NOT NULL DEFAULT 'manual' CHECK (source IN ('manual', 'ai', 'inherited')),
                    confidence  REAL,
                    created_at  INTEGER NOT NULL,
                    PRIMARY KEY (note_id, tag_id)
                );

                CREATE INDEX IF NOT EXISTS idx_note_tags_tag ON note_tags(tag_id);

                CREATE TABLE IF NOT EXISTS chunk_tags (
                    chunk_id    INTEGER NOT NULL REFERENCES chunks(chunk_id) ON DELETE CASCADE,
                    tag_id      INTEGER NOT NULL REFERENCES tags(tag_id) ON DELETE CASCADE,
                    source      TEXT NOT NULL DEFAULT 'inherited' CHECK (source IN ('manual', 'ai', 'inherited')),
                    created_at  INTEGER NOT NULL,
                    PRIMARY KEY (chunk_id, tag_id)
                );

                CREATE INDEX IF NOT EXISTS idx_chunk_tags_tag ON chunk_tags(tag_id);

                UPDATE meta SET value = '14' WHERE key = 'schema_version';"
            )?;
            version = "14".to_string();
        }

        if version == "14" {
            println!("Init: upgrading database from v14 to v15 (Lifecycle and Card enhancements)");
            conn.execute_batch("PRAGMA foreign_keys = OFF;")?;

            // 1. Migrate notes table
            conn.execute_batch(
                "CREATE TABLE notes_new (
                     note_id              INTEGER PRIMARY KEY AUTOINCREMENT,
                     title                TEXT NOT NULL DEFAULT '',
                     content              TEXT NOT NULL DEFAULT '',
                     content_hash         TEXT NOT NULL DEFAULT '',
                     parent_note_id       INTEGER REFERENCES notes_new(note_id) ON DELETE SET NULL,
                     path_cached          TEXT NOT NULL DEFAULT '',
                     sort_order           INTEGER NOT NULL DEFAULT 0,
                     icon                 TEXT,
                     indexing_status      TEXT NOT NULL DEFAULT 'pending'
                         CHECK (indexing_status IN ('pending','indexing','indexed','failed','stale')),
                     indexing_error       TEXT,
                     indexed_at           INTEGER,
                     embedding_model      TEXT,
                     embedding_dimension  INTEGER,
                     indexing_version     TEXT,
                     lifecycle            TEXT NOT NULL DEFAULT 'active' CHECK (lifecycle IN ('draft', 'active', 'archived', 'deleted')),
                     is_pinned            INTEGER NOT NULL DEFAULT 0,
                     is_favorite          INTEGER NOT NULL DEFAULT 0,
                     mastery              REAL,
                     last_studied         INTEGER,
                     created_at           INTEGER NOT NULL,
                     updated_at           INTEGER NOT NULL,
                     CHECK (parent_note_id IS NULL OR parent_note_id <> note_id)
                  );

                  INSERT INTO notes_new (
                     note_id, title, content, content_hash, parent_note_id, path_cached, sort_order, icon,
                     indexing_status, indexing_error, indexed_at, embedding_model, embedding_dimension, indexing_version,
                     lifecycle, is_pinned, is_favorite, mastery, last_studied, created_at, updated_at
                  )
                  SELECT
                     note_id, title, content, content_hash, parent_note_id, path_cached, sort_order, icon,
                     indexing_status, indexing_error, indexed_at, embedding_model, embedding_dimension, indexing_version,
                     CASE
                       WHEN is_deleted = 1 THEN 'deleted'
                       WHEN is_archived = 1 THEN 'archived'
                       WHEN is_draft = 1 THEN 'draft'
                       ELSE 'active'
                     END,
                     is_pinned, is_favorite, mastery, last_studied, created_at, updated_at
                  FROM notes;

                  DROP TABLE notes;
                  ALTER TABLE notes_new RENAME TO notes;

                  CREATE INDEX IF NOT EXISTS idx_notes_indexing_status ON notes(indexing_status) WHERE indexing_status != 'indexed';
                  CREATE INDEX IF NOT EXISTS idx_notes_active ON notes(updated_at DESC) WHERE lifecycle = 'active';
                  CREATE INDEX IF NOT EXISTS idx_notes_parent ON notes(parent_note_id) WHERE lifecycle != 'deleted';
                  CREATE INDEX IF NOT EXISTS idx_notes_path ON notes(path_cached);
                  CREATE INDEX IF NOT EXISTS idx_notes_drafts ON notes(updated_at DESC) WHERE lifecycle = 'draft';
                  CREATE INDEX IF NOT EXISTS idx_notes_pinned ON notes(updated_at DESC) WHERE is_pinned = 1 AND lifecycle != 'deleted';
                "
            )?;

            // 2. Migrate cards table
            conn.execute_batch(
                "CREATE TABLE cards_new (
                     card_id INTEGER PRIMARY KEY AUTOINCREMENT,
                     note_id INTEGER NOT NULL REFERENCES notes(note_id) ON DELETE CASCADE,
                     chunk_id INTEGER REFERENCES chunks(chunk_id) ON DELETE SET NULL,
                     card_type TEXT NOT NULL DEFAULT 'heading' CHECK (card_type IN ('heading', 'qa', 'cloze', 'manual')),
                     custom_front TEXT,
                     custom_back TEXT,
                     cloze_mask TEXT,
                     status TEXT NOT NULL DEFAULT 'fresh' CHECK (status IN ('fresh', 'stale', 'orphaned', 'suspended')),
                     last_section_snapshot TEXT,
                     generated_by TEXT,
                     source_raw_hash_at_creation TEXT,
                     created_at INTEGER NOT NULL,
                     updated_at INTEGER NOT NULL
                  );

                  INSERT INTO cards_new (
                     card_id, note_id, chunk_id, card_type, custom_front, custom_back, cloze_mask,
                     status, last_section_snapshot, generated_by, source_raw_hash_at_creation, created_at, updated_at
                  )
                  SELECT
                     card_id,
                     COALESCE((SELECT note_id FROM chunks WHERE chunk_id = cards.chunk_id), 1),
                     chunk_id,
                     card_type,
                     custom_front,
                     custom_back,
                     cloze_mask,
                     CASE
                       WHEN is_suspended = 1 THEN 'suspended'
                       WHEN is_stale = 1 THEN 'stale'
                       ELSE 'fresh'
                     END,
                     NULL,
                     generated_by,
                     section_hash_at_creation,
                     created_at,
                     updated_at
                  FROM cards;

                  DROP TABLE cards;
                  ALTER TABLE cards_new RENAME TO cards;

                  CREATE INDEX IF NOT EXISTS idx_cards_section_id ON cards(chunk_id);
                  CREATE INDEX IF NOT EXISTS idx_cards_status ON cards(status);
                  CREATE INDEX IF NOT EXISTS idx_cards_active ON cards(chunk_id) WHERE status != 'suspended';
                "
            )?;

            // 3. Add chunks BEFORE DELETE trigger to orphan cards
            conn.execute_batch(
                "CREATE TRIGGER IF NOT EXISTS chunks_orphaning_cards
                 BEFORE DELETE ON chunks
                 FOR EACH ROW
                 BEGIN
                     UPDATE cards
                     SET status = 'orphaned',
                         last_section_snapshot = OLD.raw_text
                     WHERE chunk_id = OLD.chunk_id;
                 END;
                "
            )?;

            conn.execute_batch(
                "UPDATE meta SET value = '15' WHERE key = 'schema_version';"
            )?;
            conn.execute_batch("PRAGMA foreign_keys = ON;")?;
            version = "15".to_string();
        }

        if version != "15" {
            return Err(AppError::Other(format!(
                "Unsupported database version: got {}, expected 15", version
            )));
        }
    }

    // 2. Восстанавливаем прерванные задачи индексации
    println!("Init: recover_interrupted");
    helpers::recover_interrupted(conn)?;

    Ok(())
}
