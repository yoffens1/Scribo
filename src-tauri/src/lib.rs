use parking_lot::Mutex;
use rusqlite::Connection;

mod schema;
mod error;
mod commands;

pub use error::AppError;

#[tauri::command]
fn db_initialize(state: tauri::State<'_, DbState>, db_path: String) -> Result<(), String> {
    let mut conn = Connection::open(&db_path).map_err(|e| e.to_string())?;
    conn.execute_batch("
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA foreign_keys = ON;
        PRAGMA temp_store = MEMORY;
        PRAGMA mmap_size = 30000000000;
        PRAGMA cache_size = -64000;
    ").map_err(|e| e.to_string())?;
    schema::initialize_schema(&mut conn)?;
    let mut db_guard = state.0.lock();
    *db_guard = Some(conn);
    Ok(())
}

#[tauri::command]
fn db_close(state: tauri::State<'_, DbState>) -> Result<(), String> {
    let mut db_guard = state.0.lock();
    if let Some(conn) = db_guard.take() {
        let _ = conn.close();
    }
    Ok(())
}

pub struct DbState(pub Mutex<Option<Connection>>);





#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_fs::init())
        .manage(DbState(Mutex::new(None)))
        .invoke_handler(tauri::generate_handler![
            db_initialize,
            db_close,
            commands::db::db_begin_transaction,
            commands::db::db_commit_transaction,
            commands::db::db_rollback_transaction,
            commands::db::db_vacuum,
            commands::db::db_optimize,
            commands::files::files_get_by_path,
            commands::files::files_insert_indexing,
            commands::files::files_update_indexing,
            commands::files::files_exists,
            commands::files::files_mark_indexed,
            commands::files::files_mark_failed,
            commands::files::files_insert_failed,
            commands::files::files_soft_delete,
            commands::files::files_restore,
            commands::files::files_rename,
            commands::files::files_count_chunks,
            commands::files::files_hard_delete,
            commands::files::files_get_all,
            commands::files::files_get_map,
            commands::files::files_get_by_source_file_id,
            commands::files::files_insert_minimal,
            commands::files::files_sync_upsert,
            commands::chunks::chunks_delete_by_file_id,
            commands::chunks::chunks_insert,
            commands::chunks::chunks_get_by_file_path,
            commands::chunks::chunks_get_all,
            commands::chunks::chunks_get_by_file_name,
            commands::cards::cards_insert_ignore,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
