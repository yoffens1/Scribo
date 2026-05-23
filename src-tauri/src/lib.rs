use parking_lot::{Mutex, RwLock};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;

mod commands;
mod error;
pub mod chunker;
pub mod schema;

pub use error::AppError;

/// Application database state.
///
/// `pool`       — protected by `RwLock` so that concurrent reads (every `with_conn`)
///               take a shared lock, while re-initialization (rare) takes an exclusive
///               lock. The lock is held only for the duration of the cheap Arc-clone,
///               keeping the hot path effectively lock-free under read contention.
///
/// `write_lock` — serializes all write transactions so SQLite's single-writer model
///               is enforced in Rust rather than relying on `busy_timeout`.
pub struct DbState {
    pub pool: RwLock<Option<Pool<SqliteConnectionManager>>>,
    pub write_lock: Mutex<()>,
}

impl Default for DbState {
    fn default() -> Self {
        Self::new()
    }
}

impl DbState {
    pub fn new() -> Self {
        Self {
            pool: RwLock::new(None),
            write_lock: Mutex::new(()),
        }
    }

    #[inline]
    pub fn with_conn<T>(
        &self,
        f: impl FnOnce(&mut rusqlite::Connection) -> Result<T, AppError>,
    ) -> Result<T, AppError> {
        // Read lock is held only for the Arc-clone, then immediately released.
        let pool = self.pool.read().as_ref().cloned().ok_or(AppError::NotInitialized)?;
        let mut conn = pool.get().map_err(|e| AppError::Other(e.to_string()))?;
        f(&mut conn)
    }
}

#[tauri::command]
fn db_initialize(
    state: tauri::State<'_, DbState>,
    db_path: String,
    force: bool,
) -> Result<(), AppError> {
    // Fast path: already initialized and no force-reinit requested.
    if state.inner().pool.read().is_some() && !force {
        return Ok(());
    }

    let manager = SqliteConnectionManager::file(&db_path).with_init(|conn| {
        conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA foreign_keys = ON;
            PRAGMA temp_store = MEMORY;
            PRAGMA mmap_size = 30000000000;
            PRAGMA cache_size = -64000;
            PRAGMA busy_timeout = 5000;
        ",
        )
    });

    let pool = Pool::builder()
        .max_size(10)
        .build(manager)
        .map_err(|e| AppError::Other(e.to_string()))?;

    // Run schema migrations on a dedicated connection before exposing the pool.
    {
        let mut conn = pool.get().map_err(|e| AppError::Other(e.to_string()))?;
        schema::initialize_schema(&mut conn)?;
    }

    *state.inner().pool.write() = Some(pool);
    Ok(())
}

#[tauri::command]
fn db_close(state: tauri::State<'_, DbState>) -> Result<(), AppError> {
    let mut guard = state.inner().pool.write();
    // SQLite recommends running PRAGMA optimize before closing. It updates
    // internal statistics used by the query planner and is cheap (~ms).
    if let Some(pool) = guard.as_ref() {
        if let Ok(conn) = pool.get() {
            let _ = conn.execute_batch("PRAGMA optimize;");
        }
    }
    *guard = None; // Dropping the pool closes all connections.
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_fs::init())
        .manage(DbState::new())
        .invoke_handler(tauri::generate_handler![
            db_initialize,
            db_close,
            commands::db::db_vacuum,
            commands::db::db_optimize,
            commands::files::files_get_by_path,
            commands::files::files_upsert_indexing,
            commands::files::files_mark_indexed,
            commands::files::files_mark_failed,
            commands::files::files_insert_failed,
            commands::files::files_soft_delete,
            commands::files::files_restore,
            commands::files::files_rename,
            commands::files::files_count_chunks,
            commands::files::files_hard_delete,
            commands::files::files_get_all,
            commands::files::files_get_by_source_file_id,
            commands::files::files_insert_minimal,
            commands::files::files_sync_upsert,
            commands::files::files_update_content_with_diff,
            commands::chunks::chunks_delete_by_file_id,
            commands::chunks::chunks_insert,
            commands::chunks::chunks_get_by_file_path,
            commands::chunks::chunks_get_all,
            commands::chunks::chunks_get_by_file_name,
            commands::chunks::chunks_search,
            commands::chunks::chunks_vector_search,
            commands::cards::cards_insert_ignore,
            commands::cards::cards_review_fsrs,
            commands::chunker::chunk_text_paired,
            commands::chunker::count_text_tokens,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
