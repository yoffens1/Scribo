pub mod helpers;
pub mod tables;
pub mod migrations;

use rusqlite::Connection;
use crate::error::AppError;

/// Главная функция инициализации схемы БД Scribo
pub fn initialize_schema(conn: &mut Connection) -> Result<(), AppError> {
    // 1. Проверяем целостность файла
    println!("Init: check_integrity");
    helpers::check_integrity(conn)?;

    // 2. Создаем базовые таблицы (идемпотентно)
    println!("Init: meta");
    tables::create_meta(conn)?;
    println!("Init: files");
    tables::create_files(conn)?;
    println!("Init: chunks");
    tables::create_chunks(conn)?;
    println!("Init: cards");
    tables::create_cards(conn)?;
    println!("Init: history");
    tables::create_history_and_logs(conn)?;

    // 3. Считываем версию и применяем миграции
    let current_version = migrations::get_schema_version(conn)?;
    println!("Init: apply_migrations from {}", current_version);
    migrations::apply_migrations(conn, current_version)?;

    // 4. Восстанавливаем прерванные задачи индексации
    println!("Init: recover_interrupted");
    helpers::recover_interrupted(conn)?;

    Ok(())
}
