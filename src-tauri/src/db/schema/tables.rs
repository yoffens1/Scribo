use rusqlite::Connection;
use crate::error::AppError;

pub fn create_schema(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
         );

          CREATE TABLE IF NOT EXISTS notes (
             note_id              INTEGER PRIMARY KEY AUTOINCREMENT,
             title                TEXT NOT NULL DEFAULT '',
             content              TEXT NOT NULL DEFAULT '',
             content_hash         TEXT NOT NULL DEFAULT '',
             tags                 TEXT,

             -- Иерархия
             parent_note_id       INTEGER REFERENCES notes(note_id) ON DELETE SET NULL,
             path_cached          TEXT NOT NULL DEFAULT '',
             sort_order           INTEGER NOT NULL DEFAULT 0,
             icon                 TEXT,

             -- Индексация и типы
             indexing_status      TEXT NOT NULL DEFAULT 'pending'
                 CHECK (indexing_status IN ('pending','indexing','indexed','failed','stale')),
             indexing_error       TEXT,
             indexed_at           INTEGER,
             embedding_model      TEXT,
             embedding_dimension  INTEGER,
             indexing_version     TEXT,

             is_draft             INTEGER NOT NULL DEFAULT 0,
             is_archived          INTEGER NOT NULL DEFAULT 0,
             is_deleted           INTEGER NOT NULL DEFAULT 0,
             is_pinned            INTEGER NOT NULL DEFAULT 0,
             is_favorite          INTEGER NOT NULL DEFAULT 0,

             -- Метаданные обучения
             mastery              REAL,
             last_studied         INTEGER,

             created_at           INTEGER NOT NULL DEFAULT (strftime('%s','now')),
             updated_at           INTEGER NOT NULL DEFAULT (strftime('%s','now'))
          );

          CREATE TABLE IF NOT EXISTS fragments (
             fragment_id INTEGER PRIMARY KEY AUTOINCREMENT,
             note_id INTEGER NOT NULL REFERENCES notes(note_id) ON DELETE CASCADE,
             fragment_index INTEGER NOT NULL,
             text_clean TEXT NOT NULL,
             source_hash TEXT NOT NULL,
             embedding BLOB,
             token_count INTEGER,
             UNIQUE(note_id, fragment_index)
          );

          CREATE TABLE IF NOT EXISTS sections (
             section_id INTEGER PRIMARY KEY AUTOINCREMENT,
             note_id INTEGER NOT NULL REFERENCES notes(note_id) ON DELETE CASCADE,
             section_index INTEGER NOT NULL,
             text_raw TEXT NOT NULL,
             heading TEXT,
             heading_level INTEGER,
             source_hash TEXT NOT NULL,
             created_at INTEGER NOT NULL DEFAULT (strftime('%s','now')),
             UNIQUE(note_id, section_index)
          );

          CREATE TABLE IF NOT EXISTS cards (
             card_id INTEGER PRIMARY KEY AUTOINCREMENT,
             section_id INTEGER NOT NULL REFERENCES sections(section_id) ON DELETE CASCADE,
             card_type TEXT NOT NULL DEFAULT 'heading'
                 CHECK (card_type IN ('heading', 'qa', 'cloze', 'manual')),
             custom_front TEXT,
             custom_back TEXT,
             cloze_mask TEXT,
             is_stale INTEGER NOT NULL DEFAULT 0,
             is_suspended INTEGER NOT NULL DEFAULT 0,
             generated_by TEXT,
             section_hash_at_creation TEXT,
             created_at INTEGER NOT NULL DEFAULT (strftime('%s','now')),
             updated_at INTEGER NOT NULL DEFAULT (strftime('%s','now'))
          );

          CREATE TABLE IF NOT EXISTS schedules (
             schedule_id INTEGER PRIMARY KEY AUTOINCREMENT,
             target_type TEXT NOT NULL CHECK (target_type IN ('card', 'note')),
             target_id INTEGER NOT NULL,
             state TEXT NOT NULL DEFAULT 'new'
                 CHECK (state IN ('new', 'learning', 'review', 'relearning')),
             stability REAL NOT NULL DEFAULT 0.0,
             difficulty REAL NOT NULL DEFAULT 0.0,
             reps INTEGER NOT NULL DEFAULT 0,
             lapses INTEGER NOT NULL DEFAULT 0,
             last_reviewed INTEGER,
             next_review INTEGER,
             UNIQUE(target_type, target_id)
          );

          CREATE TABLE IF NOT EXISTS note_revisions (
             history_id INTEGER PRIMARY KEY AUTOINCREMENT,
             note_id INTEGER NOT NULL REFERENCES notes(note_id) ON DELETE CASCADE,
             patch TEXT NOT NULL,
             created_at INTEGER NOT NULL
          );

          CREATE TABLE IF NOT EXISTS review_logs (
             log_id INTEGER PRIMARY KEY AUTOINCREMENT,
             schedule_id INTEGER NOT NULL REFERENCES schedules(schedule_id) ON DELETE CASCADE,
             rating INTEGER NOT NULL,
             reviewed_at INTEGER NOT NULL,
             prev_stability REAL,
             prev_difficulty REAL,
             elapsed_days INTEGER
          );

          CREATE VIRTUAL TABLE IF NOT EXISTS fragments_fts USING fts5(
             text_clean,
             content='fragments',
             content_rowid='fragment_id',
             tokenize = 'unicode61 remove_diacritics 2'
          );

          -- Triggers
          CREATE TRIGGER IF NOT EXISTS fragments_fts_insert AFTER INSERT ON fragments BEGIN
             INSERT INTO fragments_fts(rowid, text_clean) VALUES (NEW.fragment_id, NEW.text_clean);
          END;

          CREATE TRIGGER IF NOT EXISTS fragments_fts_delete AFTER DELETE ON fragments BEGIN
             INSERT INTO fragments_fts(fragments_fts, rowid, text_clean) VALUES('delete', OLD.fragment_id, OLD.text_clean);
          END;

          CREATE TRIGGER IF NOT EXISTS fragments_fts_update AFTER UPDATE OF text_clean ON fragments BEGIN
             INSERT INTO fragments_fts(fragments_fts, rowid, text_clean) VALUES('delete', OLD.fragment_id, OLD.text_clean);
             INSERT INTO fragments_fts(rowid, text_clean) VALUES (NEW.fragment_id, NEW.text_clean);
          END;

          CREATE TRIGGER IF NOT EXISTS schedules_cascade_card_delete
          AFTER DELETE ON cards
          FOR EACH ROW
          BEGIN
             DELETE FROM schedules
             WHERE target_type = 'card' AND target_id = OLD.card_id;
          END;

          CREATE TRIGGER IF NOT EXISTS schedules_cascade_note_delete
          AFTER DELETE ON notes
          FOR EACH ROW
          BEGIN
             DELETE FROM schedules
             WHERE target_type = 'note' AND target_id = OLD.note_id;
          END;

          CREATE TRIGGER IF NOT EXISTS schedules_check_target_insert
          BEFORE INSERT ON schedules
          FOR EACH ROW
          BEGIN
             SELECT CASE
                 WHEN NEW.target_type = 'card'
                      AND NOT EXISTS (SELECT 1 FROM cards WHERE card_id = NEW.target_id)
                     THEN RAISE(ABORT, 'schedule.target_id does not match an existing card')
                 WHEN NEW.target_type = 'note'
                      AND NOT EXISTS (SELECT 1 FROM notes WHERE note_id = NEW.target_id)
                     THEN RAISE(ABORT, 'schedule.target_id does not match an existing note')
             END;
          END;

          CREATE TRIGGER IF NOT EXISTS schedules_check_target_update
          BEFORE UPDATE OF target_type, target_id ON schedules
          FOR EACH ROW
          BEGIN
             SELECT CASE
                 WHEN NEW.target_type = 'card'
                      AND NOT EXISTS (SELECT 1 FROM cards WHERE card_id = NEW.target_id)
                     THEN RAISE(ABORT, 'schedule.target_id does not match an existing card')
                 WHEN NEW.target_type = 'note'
                      AND NOT EXISTS (SELECT 1 FROM notes WHERE note_id = NEW.target_id)
                     THEN RAISE(ABORT, 'schedule.target_id does not match an existing note')
             END;
          END;

          -- Indexes
          CREATE INDEX IF NOT EXISTS idx_notes_indexing_status ON notes(indexing_status)
              WHERE indexing_status != 'indexed';
          CREATE INDEX IF NOT EXISTS idx_notes_active ON notes(updated_at DESC)
              WHERE is_deleted = 0 AND is_archived = 0;
          CREATE INDEX IF NOT EXISTS idx_notes_parent ON notes(parent_note_id) WHERE is_deleted = 0;
          CREATE INDEX IF NOT EXISTS idx_notes_path ON notes(path_cached);
          CREATE INDEX IF NOT EXISTS idx_notes_drafts ON notes(updated_at DESC) WHERE is_draft = 1;
          CREATE INDEX IF NOT EXISTS idx_notes_pinned ON notes(updated_at DESC) WHERE is_pinned = 1 AND is_deleted = 0;

          CREATE INDEX IF NOT EXISTS idx_fragments_note_id ON fragments(note_id);
          CREATE INDEX IF NOT EXISTS idx_sections_note_id ON sections(note_id);
          CREATE INDEX IF NOT EXISTS idx_cards_section_id ON cards(section_id);
          CREATE INDEX IF NOT EXISTS idx_schedules_due ON schedules (next_review) WHERE next_review IS NOT NULL;
          CREATE INDEX IF NOT EXISTS idx_schedules_target ON schedules (target_type, target_id);
          CREATE INDEX IF NOT EXISTS idx_review_logs_schedule ON review_logs(schedule_id);
          CREATE INDEX IF NOT EXISTS idx_cards_stale ON cards(section_id) WHERE is_stale = 1;
          CREATE INDEX IF NOT EXISTS idx_cards_not_suspended ON cards(section_id) WHERE is_suspended = 0;
         "
    )?;
    Ok(())
}
