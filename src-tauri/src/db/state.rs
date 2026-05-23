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
