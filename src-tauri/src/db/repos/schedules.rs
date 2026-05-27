use rusqlite::{Connection, OptionalExtension};
use crate::error::AppError;
use crate::domain::{NewSchedule, ReviewTarget, ReviewTargetType, Schedule, ScheduleId, SchedulerState, Timestamp};

fn row_to_schedule(row: &rusqlite::Row) -> Result<Schedule, rusqlite::Error> {
    let id: i64 = row.get(0)?;
    let target_type_str: String = row.get(1)?;
    let target_id: i64 = row.get(2)?;
    let state_str: String = row.get(3)?;
    
    let target_type = ReviewTargetType::parse(&target_type_str)
        .unwrap_or(ReviewTargetType::Card);
    let target = ReviewTarget::from_parts(target_type, target_id);
    
    let state = SchedulerState::parse(&state_str).unwrap_or(SchedulerState::New);

    Ok(Schedule {
        id: ScheduleId(id),
        target,
        state,
        stability: row.get(4)?,
        difficulty: row.get(5)?,
        reps: row.get(6)?,
        lapses: row.get(7)?,
        last_reviewed: row.get::<_, Option<i64>>(8)?,
        next_review: row.get::<_, Option<i64>>(9)?,
    })
}

pub fn find_by_id(conn: &Connection, id: ScheduleId) -> Result<Option<Schedule>, AppError> {
    let res = conn.query_row(
        "SELECT schedule_id, target_type, target_id, state, stability, difficulty, reps, lapses, last_reviewed, next_review 
         FROM schedules WHERE schedule_id = ?",
        [id.0],
        row_to_schedule,
    ).optional()?;
    Ok(res)
}

pub fn find_by_target(conn: &Connection, target: ReviewTarget) -> Result<Option<Schedule>, AppError> {
    let res = conn.query_row(
        "SELECT schedule_id, target_type, target_id, state, stability, difficulty, reps, lapses, last_reviewed, next_review 
         FROM schedules WHERE target_type = ? AND target_id = ?",
        rusqlite::params![target.target_type().as_str(), target.target_id()],
        row_to_schedule,
    ).optional()?;
    Ok(res)
}

pub fn find_due(conn: &Connection, now: Timestamp, limit: i64) -> Result<Vec<Schedule>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT schedule_id, target_type, target_id, state, stability, difficulty, reps, lapses, last_reviewed, next_review 
         FROM schedules 
         WHERE next_review IS NOT NULL AND next_review <= ? 
         ORDER BY next_review ASC LIMIT ?"
    )?;

    let rows = stmt.query_map(rusqlite::params![now, limit], row_to_schedule)?;
    let mut res = Vec::new();
    for r in rows {
        res.push(r?);
    }
    Ok(res)
}

pub fn count_due(conn: &Connection, now: Timestamp) -> Result<i64, AppError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM schedules WHERE next_review IS NOT NULL AND next_review <= ?",
        [now],
        |r| r.get(0)
    )?;
    Ok(count)
}

pub fn insert(conn: &Connection, new: NewSchedule) -> Result<ScheduleId, AppError> {
    let id: i64 = conn.query_row(
        "INSERT INTO schedules (target_type, target_id, state, next_review) 
         VALUES (?, ?, ?, ?) RETURNING schedule_id",
        rusqlite::params![
            new.target.target_type().as_str(), 
            new.target.target_id(),
            SchedulerState::New.as_str(),
            new.initial_due
        ],
        |row| row.get(0)
    )?;
    Ok(ScheduleId(id))
}

pub fn update_state(
    conn: &Connection,
    id: ScheduleId,
    state: SchedulerState,
    stability: f64,
    difficulty: f64,
    reps: i64,
    lapses: i64,
    last_reviewed: Option<Timestamp>,
    next_review: Option<Timestamp>,
) -> Result<(), AppError> {
    conn.execute(
        "UPDATE schedules SET state = ?, stability = ?, difficulty = ?, reps = ?, lapses = ?, last_reviewed = ?, next_review = ? WHERE schedule_id = ?",
        rusqlite::params![
            state.as_str(),
            stability,
            difficulty,
            reps,
            lapses,
            last_reviewed,
            next_review,
            id.0
        ]
    )?;
    Ok(())
}

pub fn set_next_review(
    conn: &Connection,
    id: ScheduleId,
    next_review: Option<Timestamp>,
) -> Result<(), AppError> {
    conn.execute(
        "UPDATE schedules SET next_review = ? WHERE schedule_id = ?",
        rusqlite::params![next_review, id.0]
    )?;
    Ok(())
}

pub fn delete_by_target(conn: &Connection, target: ReviewTarget) -> Result<(), AppError> {
    conn.execute(
        "DELETE FROM schedules WHERE target_type = ? AND target_id = ?",
        rusqlite::params![target.target_type().as_str(), target.target_id()]
    )?;
    Ok(())
}

pub fn get_hierarchical_due_counts(
    conn: &Connection,
    now: Timestamp,
) -> Result<Vec<crate::domain::NoteDueCount>, AppError> {
    let mut stmt = conn.prepare(
        "WITH RECURSIVE note_tree(parent, descendant) AS (
            SELECT note_id, note_id
            FROM notes
            WHERE lifecycle != 'deleted'
            
            UNION ALL
            
            SELECT t.parent, n.note_id
            FROM note_tree t
            JOIN notes n ON n.parent_note_id = t.descendant
            WHERE n.lifecycle != 'deleted'
        ),
        direct_due AS (
            SELECT target_id AS note_id, COUNT(*) AS cnt
            FROM schedules
            WHERE target_type = 'note' AND next_review <= ?
            GROUP BY target_id
            UNION ALL
            SELECT c.note_id, COUNT(*) AS cnt
            FROM schedules s
            JOIN cards c ON s.target_id = c.card_id
            WHERE s.target_type = 'card' AND s.next_review <= ? AND c.status != 'suspended'
            GROUP BY c.note_id
        ),
        note_direct_due AS (
            SELECT note_id, SUM(cnt) AS direct_due_cnt
            FROM direct_due
            GROUP BY note_id
        )
        SELECT t.parent AS note_id, COALESCE(SUM(d.direct_due_cnt), 0) AS total_due_count
        FROM note_tree t
        LEFT JOIN note_direct_due d ON t.descendant = d.note_id
        GROUP BY t.parent"
    )?;

    let rows = stmt.query_map(rusqlite::params![now, now], |row| {
        Ok(crate::domain::NoteDueCount {
            note_id: row.get(0)?,
            due_count: row.get(1)?,
        })
    })?;

    let mut res = Vec::new();
    for r in rows {
        res.push(r?);
    }
    Ok(res)
}

pub fn get_repeat_mode_tree(
    conn: &Connection,
    now: Timestamp,
) -> Result<Vec<crate::domain::RepeatModeNode>, AppError> {
    let mut stmt = conn.prepare(
        "WITH RECURSIVE note_tree AS (
            SELECT n.note_id, n.title, n.parent_note_id, n.path_cached, 0 as depth
            FROM notes n
            WHERE n.parent_note_id IS NULL AND n.lifecycle = 'active'
            
            UNION ALL
            
            SELECT n.note_id, n.title, n.parent_note_id, n.path_cached, nt.depth + 1
            FROM notes n
            JOIN note_tree nt ON n.parent_note_id = nt.note_id
            WHERE n.lifecycle = 'active'
        ),
        card_counts AS (
            SELECT 
                c.note_id,
                COUNT(*) FILTER (WHERE sch.next_review <= ?) as due_count,
                COUNT(*) FILTER (WHERE sch.state = 'new') as new_count,
                COUNT(*) as total_count
            FROM cards c
            LEFT JOIN schedules sch ON sch.target_type = 'card' AND sch.target_id = c.card_id
            WHERE c.status != 'suspended'
            GROUP BY c.note_id
        )
        SELECT 
            nt.note_id,
            nt.title,
            nt.parent_note_id,
            nt.path_cached,
            nt.depth,
            COALESCE(cc.due_count, 0) as own_due,
            COALESCE(cc.total_count, 0) as own_total,
            (SELECT COALESCE(SUM(due_count), 0) 
             FROM card_counts cc2 
             JOIN notes n2 USING (note_id)
             WHERE n2.path_cached = nt.path_cached 
                OR n2.path_cached LIKE nt.path_cached || '/%') as subtree_due,
            (SELECT COALESCE(SUM(total_count), 0) 
             FROM card_counts cc2 
             JOIN notes n2 USING (note_id)
             WHERE n2.path_cached = nt.path_cached 
                OR n2.path_cached LIKE nt.path_cached || '/%') as subtree_total
        FROM note_tree nt
        LEFT JOIN card_counts cc USING (note_id)
        ORDER BY nt.path_cached"
    )?;

    let rows = stmt.query_map(rusqlite::params![now], |row| {
        Ok(crate::domain::RepeatModeNode {
            note_id: row.get(0)?,
            title: row.get(1)?,
            parent_note_id: row.get(2)?,
            path_cached: row.get(3)?,
            depth: row.get(4)?,
            own_due: row.get(5)?,
            own_total: row.get(6)?,
            subtree_due: row.get(7)?,
            subtree_total: row.get(8)?,
        })
    })?;

    let mut res = Vec::new();
    for r in rows {
        res.push(r?);
    }
    Ok(res)
}


