use crate::error::AppError;
use crate::DbState;
use tauri::State;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;

#[tauri::command]
pub fn db_initialize(
    state: State<'_, DbState>,
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
        crate::db::schema::initialize_schema(&mut conn)?;
    }

    *state.inner().pool.write() = Some(pool);
    Ok(())
}

#[tauri::command]
pub fn db_close(state: State<'_, DbState>) -> Result<(), AppError> {
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

/// VACUUM is the most expensive SQLite operation (~O(db_size)).
/// We run it in a dedicated blocking thread to avoid tying up Tauri's
/// async IPC worker pool for potentially several seconds.
#[tauri::command]
pub async fn db_vacuum(state: State<'_, DbState>) -> Result<(), AppError> {
    let pool = state.inner().pool.read().as_ref().cloned().ok_or(AppError::NotInitialized)?;

    tauri::async_runtime::spawn_blocking(move || {
        let conn = pool.get().map_err(|e| AppError::Other(e.to_string()))?;
        conn.execute_batch("VACUUM;")?;
        Ok::<(), AppError>(())
    })
    .await
    .map_err(|e| AppError::Other(e.to_string()))??;

    Ok(())
}

#[tauri::command]
pub fn db_optimize(state: State<'_, DbState>) -> Result<(), AppError> {
    state.with_conn(|conn| {
        conn.execute_batch("PRAGMA optimize;")?;
        Ok(())
    })
}

