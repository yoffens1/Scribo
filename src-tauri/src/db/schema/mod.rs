pub mod helpers;
pub mod tables;
pub mod migrations;

use rusqlite::Connection;
use crate::error::AppError;

/// Главная функция инициализации схемы БД Scribo
pub fn initialize_schema(conn: &mut Connection) -> Result<(), AppError> {
    // 1. Проверяем целостность файла
    helpers::check_integrity(conn)?;

    // 2. Создаем базовые таблицы (идемпотентно)
    tables::create_meta(conn)?;
    tables::create_files(conn)?;
    tables::create_chunks(conn)?;
    tables::create_cards(conn)?;
    tables::create_history_and_logs(conn)?;

    // 3. Считываем версию и применяем миграции
    let current_version = migrations::get_schema_version(conn)?;
    migrations::apply_migrations(conn, current_version)?;

    // 4. Восстанавливаем прерванные задачи индексации
    helpers::recover_interrupted(conn)?;

    Ok(())
}
