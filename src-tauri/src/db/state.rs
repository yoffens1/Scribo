//! # DbState — Application Database Handle
//!
//! `DbState` is the single Tauri-managed global that ties together the SQLite connection pool,
//! the LLM service cache, the vault language cache, and the write serialisation mutex.
//!
//! ## Fields
//!
//! | Field | Type | Purpose |
//! |---|---|---|
//! | `pool` | `Arc<RwLock<Option<Pool>>>` | SQLite connection pool; `None` until the user opens a vault |
//! | `reviewer` | `Arc<ReviewerService>` | Stateless FSRS review service — cached here to avoid re-allocation |
//! | `cached_vault_lang` | `Arc<RwLock<Option<String>>>` | ISO-639-1 code of the vault's dominant language, computed once per session |
//! | `llm_service` | `Arc<RwLock<Option<(LlmConfig, Arc<LlmService>)>>>` | Reuses the same LLM connection pool across requests with identical config |
//! | `write_lock` | `Mutex<()>` | Serialises all write transactions to respect SQLite's single-writer model |
//!
//! ## Concurrency model
//!
//! - **Reads** (`with_conn`): acquire a shared `RwLock` read guard, clone the `Arc<Pool>`,
//!   release the lock immediately, then borrow a connection from the pool. The hot path
//!   is effectively lock-free under read contention.
//! - **Writes** (`with_write`): first acquires `write_lock` (a `parking_lot::Mutex`) to
//!   serialise writers at the Rust level, then delegates to `with_conn`. This avoids
//!   relying on SQLite's `PRAGMA busy_timeout` for write contention.

use parking_lot::{Mutex, RwLock};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use crate::error::AppError;

pub struct DbState {
    /// SQLite r2d2 connection pool. `None` until a vault is opened via `db_initialize`.
    /// Protected by `RwLock` so the pool pointer can be swapped (e.g. user changes workspace)
    /// without blocking ongoing reads.
    pub pool: std::sync::Arc<RwLock<Option<Pool<SqliteConnectionManager>>>>,

    /// Stateless FSRS review service. Stored here to avoid creating a new instance per request.
    pub reviewer: std::sync::Arc<crate::services::reviewer::ReviewerService>,

    /// Cached vault language (ISO-639-1). Populated on first retrieval, invalidated on vault change.
    /// Avoids running `whatlang` detection on every retrieval request.
    pub cached_vault_lang: std::sync::Arc<RwLock<Option<String>>>,

    /// Cached LLM service keyed by `LlmConfig`.
    /// Stores `(config, service)` so that a config change invalidates the cache and forces
    /// a new connection pool to be created.
    pub llm_service: std::sync::Arc<RwLock<Option<(crate::ai::LlmConfig, std::sync::Arc<crate::ai::LlmService>)>>>,

    /// Serialises write transactions to enforce SQLite's single-writer constraint in Rust,
    /// rather than relying on `PRAGMA busy_timeout`.
    pub write_lock: Mutex<()>,
}

impl Default for DbState {
    fn default() -> Self {
        Self::new()
    }
}

impl DbState {
    /// Creates a new `DbState` with no open pool.
    /// The pool is initialised later by the `db_initialize` Tauri command.
    pub fn new() -> Self {
        let pool = std::sync::Arc::new(RwLock::new(None));
        let reviewer = std::sync::Arc::new(crate::services::reviewer::ReviewerService::new());
        let cached_vault_lang = std::sync::Arc::new(RwLock::new(None));
        let llm_service = std::sync::Arc::new(RwLock::new(None));

        Self {
            pool,
            reviewer,
            cached_vault_lang,
            llm_service,
            write_lock: Mutex::new(()),
        }
    }

    /// Returns the cached `LlmService` if the stored config matches `config`,
    /// or creates and caches a new one otherwise.
    ///
    /// The write lock on `llm_service` is held only for the duration of the cache lookup
    /// and optional update — not for the entire LLM request lifetime.
    pub fn get_llm_service(
        &self,
        config: &crate::ai::LlmConfig,
        app: Option<tauri::AppHandle>,
    ) -> std::sync::Arc<crate::ai::LlmService> {
        let mut guard = self.llm_service.write();
        if let Some((ref cached_config, ref service)) = *guard {
            if cached_config == config {
                return service.clone(); // Cache hit — reuse existing connection pool
            }
        }
        // Config changed or first call — create a new service and cache it
        let service = std::sync::Arc::new(crate::ai::LlmService::new(config.clone(), app));
        *guard = Some((config.clone(), service.clone()));
        service
    }

    /// Borrows a connection from the pool and calls `f` with it.
    ///
    /// Returns `AppError::NotInitialized` if the pool has not been opened yet.
    /// The `RwLock` read guard is held only long enough to clone the `Arc<Pool>`.
    #[inline]
    pub fn with_conn<T>(
        &self,
        f: impl FnOnce(&mut rusqlite::Connection) -> Result<T, AppError>,
    ) -> Result<T, AppError> {
        // Read lock is held only for the Arc-clone, then immediately released
        let pool = self.pool.read().as_ref().cloned().ok_or(AppError::NotInitialized)?;
        let mut conn = pool.get().map_err(|e| AppError::Other(e.to_string()))?;
        f(&mut conn)
    }

    /// Like `with_conn`, but first acquires `write_lock` to serialise concurrent writers.
    /// Use this for any operation that opens a SQLite `BEGIN` transaction.
    #[inline]
    pub fn with_write<T>(
        &self,
        f: impl FnOnce(&mut rusqlite::Connection) -> Result<T, AppError>,
    ) -> Result<T, AppError> {
        let _guard = self.write_lock.lock(); // Held for the duration of f()
        self.with_conn(f)
    }
}
