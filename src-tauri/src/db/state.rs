use parking_lot::{Mutex, RwLock};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use crate::error::AppError;

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
    // We use Option because the Tauri app boots up before the user selects a vault.
    // The pool is initialized dynamically at runtime via `db_initialize` and can be
    // swapped if the user changes workspaces.
    pub pool: std::sync::Arc<RwLock<Option<Pool<SqliteConnectionManager>>>>,
    pub reviewer: std::sync::Arc<crate::services::reviewer::ReviewerService>,
    pub write_lock: Mutex<()>,
}

impl Default for DbState {
    fn default() -> Self {
        Self::new()
    }
}

impl DbState {
    pub fn new() -> Self {
        let pool = std::sync::Arc::new(RwLock::new(None));
        let reviewer = std::sync::Arc::new(crate::services::reviewer::ReviewerService::new());

        Self {
            pool,
            reviewer,
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

    #[inline]
    pub fn with_write<T>(
        &self,
        f: impl FnOnce(&mut rusqlite::Connection) -> Result<T, AppError>,
    ) -> Result<T, AppError> {
        let _guard = self.write_lock.lock();
        self.with_conn(f)
    }
}
