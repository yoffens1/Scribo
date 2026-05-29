//! # Meta Repository
//!
//! Handles configuration key-value storage in the `meta` table.

use rusqlite::Connection;
use crate::error::AppError;

/// Returns the string value for a given key, if it exists.
pub fn get_value(conn: &Connection, key: &str) -> Result<Option<String>, AppError> {
    let mut stmt = conn.prepare("SELECT value FROM meta WHERE key = ?1")?;
    let mut rows = stmt.query([key])?;
    if let Some(row) = rows.next()? {
        let val: String = row.get(0)?;
        Ok(Some(val))
    } else {
        Ok(None)
    }
}

/// Returns the parsed f32 value for a given key.
pub fn get_f32(conn: &Connection, key: &str) -> Result<Option<f32>, AppError> {
    match get_value(conn, key)? {
        Some(val) => Ok(val.parse::<f32>().ok()),
        None => Ok(None),
    }
}

/// Sets the value for a given key.
pub fn set_value(conn: &Connection, key: &str, value: &str) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO meta (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        rusqlite::params![key, value],
    )?;
    Ok(())
}
