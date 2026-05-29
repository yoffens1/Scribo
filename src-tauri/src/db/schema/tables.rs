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

             lifecycle            TEXT NOT NULL DEFAULT 'active' CHECK (lifecycle IN ('draft', 'active', 'archived', 'deleted')),
             is_pinned            INTEGER NOT NULL DEFAULT 0,
             is_favorite          INTEGER NOT NULL DEFAULT 0,

             -- Метаданные обучения
             mastery              REAL,
             last_studied         INTEGER,

             created_at           INTEGER NOT NULL DEFAULT (strftime('%s','now')),
             updated_at           INTEGER NOT NULL DEFAULT (strftime('%s','now')),

             CHECK (parent_note_id IS NULL OR parent_note_id <> note_id)
          );

          CREATE TABLE IF NOT EXISTS fragments (
             fragment_id             INTEGER PRIMARY KEY AUTOINCREMENT,
             note_id                 INTEGER NOT NULL REFERENCES notes(note_id) ON DELETE CASCADE,
             order_index             INTEGER NOT NULL,
             
             raw_text                TEXT NOT NULL,
             raw_text_hash           TEXT NOT NULL,
             clean_text              TEXT NOT NULL,
             clean_text_hash         TEXT NOT NULL,
             
             created_at              INTEGER NOT NULL DEFAULT (strftime('%s','now')),
             updated_at              INTEGER NOT NULL DEFAULT (strftime('%s','now'))
          );

          CREATE TABLE IF NOT EXISTS fragment_embeddings (
             fragment_id             INTEGER NOT NULL REFERENCES fragments(fragment_id) ON DELETE CASCADE,
             embedding_model         TEXT NOT NULL,
             embedding_model_version TEXT NOT NULL,
             dim                     INTEGER NOT NULL,
             embedding               BLOB NOT NULL,
             embedded_at             INTEGER NOT NULL DEFAULT (strftime('%s','now')),
             PRIMARY KEY (fragment_id, embedding_model, embedding_model_version)
          );

          CREATE TABLE IF NOT EXISTS embedding_cache (
             clean_text_hash         TEXT NOT NULL,
             embedding_model         TEXT NOT NULL,
             embedding_model_version TEXT NOT NULL,
             dim                     INTEGER NOT NULL,
             embedding               BLOB NOT NULL,
             created_at              INTEGER NOT NULL DEFAULT (strftime('%s','now')),
             PRIMARY KEY (clean_text_hash, embedding_model, embedding_model_version)
          );

          CREATE TABLE IF NOT EXISTS cards (
             card_id INTEGER PRIMARY KEY AUTOINCREMENT,
             note_id INTEGER NOT NULL REFERENCES notes(note_id) ON DELETE CASCADE,
             section_id INTEGER REFERENCES fragments(fragment_id) ON DELETE SET NULL,
             card_type TEXT NOT NULL DEFAULT 'heading'
                 CHECK (card_type IN ('heading', 'qa', 'cloze', 'manual')),
             custom_front TEXT,
             custom_back TEXT,
             cloze_mask TEXT,
             status TEXT NOT NULL DEFAULT 'fresh'
                 CHECK (status IN ('fresh', 'stale', 'orphaned', 'suspended')),
             last_section_snapshot TEXT,
             generated_by TEXT,
             source_raw_hash_at_creation TEXT,
             created_at INTEGER NOT NULL DEFAULT (strftime('%s','now')),
             updated_at INTEGER NOT NULL DEFAULT (strftime('%s','now'))
          );

          CREATE TABLE IF NOT EXISTS schedules (
             schedule_id INTEGER PRIMARY KEY AUTOINCREMENT,
             target_type TEXT NOT NULL CHECK (target_type IN ('card', 'note')),
             target_id INTEGER NOT NULL,
             
             state INTEGER NOT NULL DEFAULT 0,
             stability REAL NOT NULL DEFAULT 0.0,
             difficulty REAL NOT NULL DEFAULT 0.0,
             elapsed_days REAL NOT NULL DEFAULT 0.0,
             scheduled_days REAL NOT NULL DEFAULT 0.0,
             reps INTEGER NOT NULL DEFAULT 0,
             lapses INTEGER NOT NULL DEFAULT 0,
             last_reviewed INTEGER,
             next_review INTEGER
          );

          CREATE TABLE IF NOT EXISTS note_revisions (
             revision_id INTEGER PRIMARY KEY AUTOINCREMENT,
             note_id INTEGER NOT NULL REFERENCES notes(note_id) ON DELETE CASCADE,
             patch TEXT NOT NULL,
             created_at INTEGER NOT NULL DEFAULT (strftime('%s','now'))
          );

          CREATE TABLE IF NOT EXISTS distribution_runs (
             run_id              INTEGER PRIMARY KEY AUTOINCREMENT,
             draft_id            INTEGER NOT NULL REFERENCES notes(note_id) ON DELETE CASCADE,
             plan_json           TEXT NOT NULL,
             result_json         TEXT,
             generator_version   TEXT NOT NULL,
             status              TEXT NOT NULL CHECK (status IN ('analyzed', 'applied', 'cancelled')),
             created_at          INTEGER NOT NULL DEFAULT (strftime('%s','now')),
             applied_at          INTEGER
          );

          CREATE TABLE IF NOT EXISTS review_logs (
             log_id INTEGER PRIMARY KEY AUTOINCREMENT,
             schedule_id INTEGER NOT NULL REFERENCES schedules(schedule_id) ON DELETE CASCADE,
             rating INTEGER NOT NULL,
             reviewed_at INTEGER NOT NULL DEFAULT (strftime('%s','now')),
             
             prev_state INTEGER,
             prev_stability REAL,
             prev_difficulty REAL,
             elapsed_days INTEGER
          );

          CREATE VIRTUAL TABLE IF NOT EXISTS fragments_fts USING fts5(
             clean_text,
             content='fragments',
             content_rowid='fragment_id',
             tokenize = 'trigram'
          );

          -- Triggers
          CREATE TRIGGER IF NOT EXISTS fragments_fts_insert AFTER INSERT ON fragments BEGIN
             INSERT INTO fragments_fts(rowid, clean_text) VALUES (NEW.fragment_id, NEW.clean_text);
          END;

          CREATE TRIGGER IF NOT EXISTS fragments_fts_delete AFTER DELETE ON fragments BEGIN
             INSERT INTO fragments_fts(fragments_fts, rowid, clean_text) VALUES('delete', OLD.fragment_id, OLD.clean_text);
          END;

          CREATE TRIGGER IF NOT EXISTS fragments_fts_update AFTER UPDATE OF clean_text ON fragments BEGIN
             INSERT INTO fragments_fts(fragments_fts, rowid, clean_text) VALUES('delete', OLD.fragment_id, OLD.clean_text);
             INSERT INTO fragments_fts(rowid, clean_text) VALUES (NEW.fragment_id, NEW.clean_text);
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

          CREATE TRIGGER IF NOT EXISTS review_logs_cascade_schedule_delete
          AFTER DELETE ON schedules
          FOR EACH ROW
          BEGIN
             DELETE FROM review_logs
             WHERE schedule_id = OLD.schedule_id;
          END;

          CREATE TRIGGER IF NOT EXISTS cards_status_orphaned_on_fragment_delete
          BEFORE DELETE ON fragments
          FOR EACH ROW
          BEGIN
             UPDATE cards
             SET status = 'orphaned',
                 last_section_snapshot = OLD.raw_text
             WHERE section_id = OLD.fragment_id;
          END;

          -- Indexes
          CREATE INDEX IF NOT EXISTS idx_notes_indexing_status ON notes(indexing_status)
              WHERE indexing_status != 'indexed';
          CREATE INDEX IF NOT EXISTS idx_notes_active ON notes(updated_at DESC)
              WHERE lifecycle = 'active';
          CREATE INDEX IF NOT EXISTS idx_notes_parent ON notes(parent_note_id) WHERE lifecycle != 'deleted';
          CREATE INDEX IF NOT EXISTS idx_notes_path ON notes(path_cached);
          CREATE INDEX IF NOT EXISTS idx_notes_drafts ON notes(updated_at DESC) WHERE lifecycle = 'draft';
          CREATE INDEX IF NOT EXISTS idx_notes_pinned ON notes(updated_at DESC) WHERE is_pinned = 1 AND lifecycle != 'deleted';

          CREATE INDEX IF NOT EXISTS idx_fragments_clean_hash ON fragments(clean_text_hash);
          -- Prevents duplicate fragments with identical text within the same note.
          CREATE UNIQUE INDEX IF NOT EXISTS idx_fragments_note_leaf_hash
              ON fragments(note_id, clean_text_hash);
          CREATE INDEX IF NOT EXISTS idx_fragment_emb_model ON fragment_embeddings(embedding_model, embedding_model_version);
          
          CREATE INDEX IF NOT EXISTS idx_cards_section_id ON cards(section_id);
          CREATE INDEX IF NOT EXISTS idx_schedules_due ON schedules (next_review) WHERE next_review IS NOT NULL;
          CREATE INDEX IF NOT EXISTS idx_schedules_target ON schedules (target_type, target_id);
          CREATE INDEX IF NOT EXISTS idx_review_logs_schedule ON review_logs(schedule_id);
          CREATE INDEX IF NOT EXISTS idx_cards_status ON cards(status);
          CREATE INDEX IF NOT EXISTS idx_cards_active ON cards(section_id) WHERE status != 'suspended';

          -- Tag System
          CREATE TABLE IF NOT EXISTS tags (
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

          CREATE TABLE IF NOT EXISTS fragment_tags (
              fragment_id INTEGER NOT NULL REFERENCES fragments(fragment_id) ON DELETE CASCADE,
              tag_id      INTEGER NOT NULL REFERENCES tags(tag_id) ON DELETE CASCADE,
              source      TEXT NOT NULL DEFAULT 'inherited' CHECK (source IN ('manual', 'ai', 'inherited')),
              created_at  INTEGER NOT NULL,
              PRIMARY KEY (fragment_id, tag_id)
          );

          CREATE INDEX IF NOT EXISTS idx_fragment_tags_tag ON fragment_tags(tag_id);

          CREATE TABLE IF NOT EXISTS retrieval_calibration (
              calibration_id       INTEGER PRIMARY KEY AUTOINCREMENT,
              query                TEXT NOT NULL,
              expected_note_title  TEXT NOT NULL,
              relevance_weight     REAL NOT NULL DEFAULT 1.0,
              UNIQUE(query, expected_note_title)
          );

          CREATE TABLE IF NOT EXISTS llm_cache (
              query             TEXT NOT NULL,
              model_id          TEXT NOT NULL,
              cache_type        TEXT NOT NULL CHECK (cache_type IN ('hyde', 'translation')),
              target_lang       TEXT NOT NULL,
              cached_response   TEXT NOT NULL,
              created_at        INTEGER NOT NULL DEFAULT (strftime('%s','now')),
              PRIMARY KEY (query, model_id, cache_type, target_lang)
          );
         "
    )?;
    Ok(())
}
