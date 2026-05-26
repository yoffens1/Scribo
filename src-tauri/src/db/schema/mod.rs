pub mod helpers;
pub mod tables;
pub mod migrations;

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
        println!("Init: fresh database, creating all tables directly at v11");
        tables::create_all_v11(conn)?;
        migrations::set_schema_version(conn, 11)?;
    } else {
        // Существующая БД — считываем версию и применяем миграции поэтапно.
        let current_version = migrations::get_schema_version(conn)?;
        println!("Init: existing database, apply_migrations from {}", current_version);
        migrations::apply_migrations(conn, current_version)?;
    }

    // 4. Восстанавливаем прерванные задачи индексации
    println!("Init: recover_interrupted");
    helpers::recover_interrupted(conn)?;

    // 5. Выполняем заполнение данных (title и content) после миграции
    println!("Init: backfill_notes_after_migration");
    helpers::backfill_notes_after_migration(conn)?;

    Ok(())
}
