-- ============================================================================
-- Migration v9: extract FSRS scheduling state into a dedicated `schedules`
-- table with a polymorphic target (card | note).
--
-- Rationale:
--   * FSRS state (stability, difficulty, reps, ...) currently lives inline on
--     `cards`. That hard-couples scheduling to cards and prevents whole-note
--     reviews ("remind me about this article in a month"), since `notes` has
--     no FSRS columns.
--   * Extracting state into a separate `schedules` table with a (target_type,
--     target_id) pair makes the same FSRS engine drive both kinds of reviews.
--
-- Strategy:
--   * Create `schedules` and copy data from `cards` (target_type = 'card').
--   * Repoint `review_logs` from `card_id` to `schedule_id`.
--   * Drop FSRS columns from `cards` AFTER the data copy succeeds.
--
-- Safety:
--   * The whole migration runs inside a transaction. SQLite migrations should
--     wrap this in BEGIN/COMMIT at the application layer.
--   * Down migration (v9_down.sql) restores the original layout if the user
--     rolls back. ALL FSRS history beyond `cards` would be lost on rollback,
--     so taking a backup before applying is recommended (do this in code).
-- ============================================================================

-- 1. Create the new schedules table.
CREATE TABLE IF NOT EXISTS schedules (
    schedule_id     INTEGER PRIMARY KEY AUTOINCREMENT,
    target_type     TEXT NOT NULL CHECK (target_type IN ('card', 'note')),
    target_id       INTEGER NOT NULL,

    state           TEXT NOT NULL DEFAULT 'new'
                    CHECK (state IN ('new', 'learning', 'review', 'relearning')),
    stability       REAL NOT NULL DEFAULT 0.0,
    difficulty      REAL NOT NULL DEFAULT 0.0,
    reps            INTEGER NOT NULL DEFAULT 0,
    lapses          INTEGER NOT NULL DEFAULT 0,

    last_reviewed   INTEGER,
    next_review     INTEGER,

    UNIQUE (target_type, target_id)
);

CREATE INDEX IF NOT EXISTS idx_schedules_due
    ON schedules (next_review)
    WHERE next_review IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_schedules_target
    ON schedules (target_type, target_id);

-- 2. Copy existing per-card FSRS state into `schedules`.
--    NOTE: column names below match the assumed v8 schema. Adjust if your
--    actual `cards` columns differ (e.g. you might have used `interval`
--    instead of `next_review`).
INSERT INTO schedules (
    target_type, target_id,
    state, stability, difficulty, reps, lapses,
    last_reviewed, next_review
)
SELECT
    'card',
    card_id,
    COALESCE(state, 'new'),
    COALESCE(stability, 0.0),
    COALESCE(difficulty, 0.0),
    COALESCE(reps, 0),
    COALESCE(lapses, 0),
    last_reviewed,
    next_review
FROM cards;

-- 3. Add a `schedule_id` column to `review_logs` and backfill it.
ALTER TABLE review_logs ADD COLUMN schedule_id INTEGER;

UPDATE review_logs
SET schedule_id = (
    SELECT s.schedule_id
    FROM schedules s
    WHERE s.target_type = 'card' AND s.target_id = review_logs.card_id
);

-- 4. New columns to capture pre-review snapshots (used by FSRS re-fitting).
ALTER TABLE review_logs ADD COLUMN prev_stability REAL;
ALTER TABLE review_logs ADD COLUMN prev_difficulty REAL;
ALTER TABLE review_logs ADD COLUMN elapsed_days INTEGER;

-- 5. Drop FSRS columns from `cards`. SQLite ≥ 3.35 supports DROP COLUMN
--    directly. If targeting older SQLite, recreate the table without them.
ALTER TABLE cards DROP COLUMN state;
ALTER TABLE cards DROP COLUMN stability;
ALTER TABLE cards DROP COLUMN difficulty;
ALTER TABLE cards DROP COLUMN reps;
ALTER TABLE cards DROP COLUMN lapses;
ALTER TABLE cards DROP COLUMN last_reviewed;
ALTER TABLE cards DROP COLUMN next_review;

-- 6. Add front/back columns to cards if they don't exist yet. These are the
--    actual question/answer pair shown during review (Anki-style).
ALTER TABLE cards ADD COLUMN front TEXT;
ALTER TABLE cards ADD COLUMN back TEXT;
ALTER TABLE cards ADD COLUMN card_type TEXT NOT NULL DEFAULT 'basic'
    CHECK (card_type IN ('basic', 'reverse', 'cloze'));
ALTER TABLE cards ADD COLUMN source_fragment_id INTEGER;
ALTER TABLE cards ADD COLUMN source_offset INTEGER;
ALTER TABLE cards ADD COLUMN source_length INTEGER;
ALTER TABLE cards ADD COLUMN generated_by TEXT;
ALTER TABLE cards ADD COLUMN is_suspended INTEGER NOT NULL DEFAULT 0;

-- For pre-existing rows that lack a real Q/A, use placeholders. The user can
-- regenerate them via the AI card generator later.
UPDATE cards
SET front = COALESCE(front, '(no front — please regenerate)'),
    back  = COALESCE(back,  '(no back — please regenerate)')
WHERE front IS NULL OR back IS NULL;

-- 7. Triggers to keep the polymorphic FK honest. SQLite has no native
--    polymorphic FK, so we enforce integrity at write time.
DROP TRIGGER IF EXISTS schedules_check_target_insert;
CREATE TRIGGER schedules_check_target_insert
BEFORE INSERT ON schedules
FOR EACH ROW
BEGIN
    SELECT CASE
        WHEN NEW.target_type = 'card'
             AND NOT EXISTS (SELECT 1 FROM cards WHERE card_id = NEW.target_id)
            THEN RAISE(ABORT, 'schedule.target_id does not match an existing card')
        WHEN NEW.target_type = 'note'
             AND NOT EXISTS (SELECT 1 FROM files WHERE file_id = NEW.target_id)
            THEN RAISE(ABORT, 'schedule.target_id does not match an existing note')
    END;
END;

DROP TRIGGER IF EXISTS schedules_check_target_update;
CREATE TRIGGER schedules_check_target_update
BEFORE UPDATE OF target_type, target_id ON schedules
FOR EACH ROW
BEGIN
    SELECT CASE
        WHEN NEW.target_type = 'card'
             AND NOT EXISTS (SELECT 1 FROM cards WHERE card_id = NEW.target_id)
            THEN RAISE(ABORT, 'schedule.target_id does not match an existing card')
        WHEN NEW.target_type = 'note'
             AND NOT EXISTS (SELECT 1 FROM files WHERE file_id = NEW.target_id)
            THEN RAISE(ABORT, 'schedule.target_id does not match an existing note')
    END;
END;

-- Cascade deletes: when a card or note is removed, drop its schedule.
DROP TRIGGER IF EXISTS schedules_cascade_card_delete;
CREATE TRIGGER schedules_cascade_card_delete
AFTER DELETE ON cards
FOR EACH ROW
BEGIN
    DELETE FROM schedules
    WHERE target_type = 'card' AND target_id = OLD.card_id;
END;

DROP TRIGGER IF EXISTS schedules_cascade_note_delete;
CREATE TRIGGER schedules_cascade_note_delete
AFTER DELETE ON files
FOR EACH ROW
BEGIN
    DELETE FROM schedules
    WHERE target_type = 'note' AND target_id = OLD.file_id;
END;
