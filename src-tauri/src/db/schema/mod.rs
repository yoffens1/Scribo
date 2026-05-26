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
        println!("Init: fresh database, creating all tables directly at v1");
        tables::create_schema(conn)?;
        conn.execute(
            "INSERT INTO meta (key, value) VALUES ('schema_version', '1')",
            [],
        )?;
    } else {
        // Существующая БД — проверяем версию.
        let version: String = conn.query_row(
            "SELECT value FROM meta WHERE key = 'schema_version'",
            [],
            |r| r.get(0)
        )?;
        if version != "1" {
            return Err(AppError::Other(format!(
                "Unsupported database version: got {}, expected 1", version
            )));
        }
    }

    // 2. Восстанавливаем прерванные задачи индексации
    println!("Init: recover_interrupted");
    helpers::recover_interrupted(conn)?;

    Ok(())
}
