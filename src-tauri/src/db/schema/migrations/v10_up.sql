-- ============================================================================
-- Migration v10: rename domain entities to match the new vocabulary.
--
--   files          → notes
--   chunks         → fragments
--   files_history  → note_revisions
--   chunks_fts     → fragments_fts
--
-- This migration is NAMING-ONLY: column types and constraints are preserved.
-- Foreign keys and triggers from v9 are recreated to point at the new names.
--
-- IMPORTANT: SQLite cannot rename a virtual FTS table. We DROP and recreate
-- `chunks_fts` as `fragments_fts`, then repopulate it from the data.
-- This is fast for typical knowledge bases (rebuilding 100k rows takes
-- seconds) and avoids hidden inconsistency.
-- ============================================================================
-- 1. Drop FTS triggers BEFORE renaming tables, because SQLite's schema validation
-- can complain about missing columns/tables in triggers during the rename operations.
DROP TRIGGER IF EXISTS chunks_fts_insert;
DROP TRIGGER IF EXISTS chunks_fts_delete;
DROP TRIGGER IF EXISTS chunks_fts_update;

-- 2. Rename core tables.
ALTER TABLE files          RENAME TO notes;
ALTER TABLE chunks         RENAME TO fragments;
ALTER TABLE files_history  RENAME TO note_revisions;

-- 2. Rename primary-key columns to match the new entity vocabulary.
--    SQLite supports column renaming since 3.25.
ALTER TABLE notes          RENAME COLUMN file_id     TO note_id;
ALTER TABLE notes          RENAME COLUMN status      TO indexing_status;
ALTER TABLE notes          RENAME COLUMN last_error  TO indexing_error;
ALTER TABLE notes          RENAME COLUMN source_file_id TO source_note_id;
ALTER TABLE notes          RENAME COLUMN chunking_version TO fragmenting_version;
ALTER TABLE fragments      RENAME COLUMN chunk_id    TO fragment_id;
ALTER TABLE fragments      RENAME COLUMN chunk_index TO fragment_index;
ALTER TABLE fragments      RENAME COLUMN chunk_text  TO text;
ALTER TABLE fragments      RENAME COLUMN file_id     TO note_id;
ALTER TABLE note_revisions RENAME COLUMN file_id     TO note_id;

-- 3. Add `title`, `content` and `tags` columns to notes
ALTER TABLE notes ADD COLUMN title TEXT NOT NULL DEFAULT '';
ALTER TABLE notes ADD COLUMN content TEXT NOT NULL DEFAULT '';
ALTER TABLE notes ADD COLUMN tags TEXT;

-- 4. Update card foreign keys to point at the new names.
--    Cards already reference notes by id; the column was `file_id`, rename it.
ALTER TABLE cards RENAME COLUMN file_id TO note_id;

-- 5. Drop and recreate the FTS5 virtual table under the new name.
DROP TABLE IF EXISTS chunks_fts;

CREATE VIRTUAL TABLE fragments_fts USING fts5(
    text,
    content='fragments',
    content_rowid='fragment_id'
);

-- Repopulate the FTS index from the renamed `fragments` table.
INSERT INTO fragments_fts(rowid, text)
SELECT fragment_id, text FROM fragments;

CREATE TRIGGER fragments_fts_insert AFTER INSERT ON fragments BEGIN
    INSERT INTO fragments_fts(rowid, text) VALUES (NEW.fragment_id, NEW.text);
END;

CREATE TRIGGER fragments_fts_delete AFTER DELETE ON fragments BEGIN
    INSERT INTO fragments_fts(fragments_fts, rowid, text) VALUES('delete', OLD.fragment_id, OLD.text);
END;

CREATE TRIGGER fragments_fts_update AFTER UPDATE OF text ON fragments BEGIN
    INSERT INTO fragments_fts(fragments_fts, rowid, text) VALUES('delete', OLD.fragment_id, OLD.text);
    INSERT INTO fragments_fts(rowid, text) VALUES (NEW.fragment_id, NEW.text);
END;

-- 6. Recreate the v9 schedules triggers to point at `notes` (was `files`).
DROP TRIGGER IF EXISTS schedules_check_target_insert;
DROP TRIGGER IF EXISTS schedules_check_target_update;
DROP TRIGGER IF EXISTS schedules_cascade_note_delete;

CREATE TRIGGER schedules_check_target_insert
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

CREATE TRIGGER schedules_check_target_update
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

CREATE TRIGGER schedules_cascade_note_delete
AFTER DELETE ON notes
FOR EACH ROW
BEGIN
    DELETE FROM schedules
    WHERE target_type = 'note' AND target_id = OLD.note_id;
END;

-- 7. Recreate any indexes that referenced old names. Adjust if your v8
--    schema had additional indexes — list them here.
DROP INDEX IF EXISTS idx_chunks_file_id;
CREATE INDEX IF NOT EXISTS idx_fragments_note_id ON fragments(note_id);

DROP INDEX IF EXISTS idx_files_path;
CREATE INDEX IF NOT EXISTS idx_notes_file_path ON notes(file_path);

DROP INDEX IF EXISTS idx_files_history_file_id;
CREATE INDEX IF NOT EXISTS idx_note_revisions_note_id ON note_revisions(note_id);
