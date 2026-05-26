use rusqlite::Connection;
use crate::error::AppError;





pub fn insert_ignore(conn: &Connection, note_id: i64) -> Result<(), AppError> {
    conn.execute(
        "INSERT OR IGNORE INTO cards (note_id) VALUES (?)",
        rusqlite::params![note_id],
    )?;
    let card_id = conn.last_insert_rowid();
    conn.execute(
        "INSERT OR IGNORE INTO schedules (target_type, target_id, state)
         VALUES ('card', ?, 'new')",
        rusqlite::params![card_id],
    )?;
    Ok(())
}

