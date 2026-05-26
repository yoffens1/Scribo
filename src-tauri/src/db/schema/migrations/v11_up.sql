-- Migration v11: Add CHECK constraint on indexing_status in notes table.
PRAGMA foreign_keys = OFF;

-- Drop triggers referencing `notes` before dropping the table to avoid schema validation errors in SQLite.
DROP TRIGGER IF EXISTS schedules_check_target_insert;
DROP TRIGGER IF EXISTS schedules_check_target_update;
DROP TRIGGER IF EXISTS schedules_cascade_note_delete;

CREATE TABLE notes_new (
    note_id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_path TEXT NOT NULL UNIQUE,
    file_name TEXT NOT NULL,
    file_hash TEXT,
    file_mtime INTEGER,
    embedding_model TEXT DEFAULT 'unknown',
    embedding_dimension INTEGER,
    indexing_version TEXT DEFAULT '1',
    source_note_id INTEGER REFERENCES notes_new(note_id) ON DELETE SET NULL,
    is_deleted INTEGER DEFAULT 0,
    indexing_status TEXT DEFAULT 'indexed'
        CHECK (indexing_status IN ('pending', 'indexing', 'indexed', 'failed', 'stale')),
    indexing_error TEXT,
    updated_at INTEGER,
    indexed_at INTEGER,
    title TEXT NOT NULL DEFAULT '',
    content TEXT NOT NULL DEFAULT '',
    tags TEXT
);

INSERT INTO notes_new (
    note_id, file_path, file_name, file_hash, file_mtime,
    embedding_model, embedding_dimension, indexing_version,
    source_note_id, is_deleted, indexing_status, indexing_error,
    updated_at, indexed_at, title, content, tags
)
SELECT 
    note_id, file_path, file_name, file_hash, file_mtime,
    embedding_model, embedding_dimension, fragmenting_version,
    source_note_id, is_deleted, indexing_status, indexing_error,
    updated_at, indexed_at, title, content, tags
FROM notes;

DROP TABLE notes;
ALTER TABLE notes_new RENAME TO notes;

PRAGMA foreign_keys = ON;

-- Recreate trigger schedules_cascade_note_delete
CREATE TRIGGER schedules_cascade_note_delete
AFTER DELETE ON notes
FOR EACH ROW
BEGIN
    DELETE FROM schedules
    WHERE target_type = 'note' AND target_id = OLD.note_id;
END;

-- Recreate schedules verification triggers
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

-- Recreate indexes
CREATE INDEX IF NOT EXISTS idx_notes_file_path ON notes(file_path);
CREATE INDEX IF NOT EXISTS idx_notes_deleted_status ON notes(is_deleted, indexing_status);
