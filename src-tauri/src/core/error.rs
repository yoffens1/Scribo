//! # Scribo Error Types
//!
//! Defines the backend-wide error representation, `AppError`.
//! Errors implement `serde::Serialize` to be easily passed across Tauri's IPC border
//! to the frontend as rejected promises.

use serde::Serialize;

/// App-wide error representation mapping system errors, DB failures, and custom errors.
#[derive(thiserror::Error, Debug)]
pub enum AppError {
    /// SQLite database engine errors.
    #[error("DB error: {0}")]
    Db(#[from] rusqlite::Error),

    /// Standard I/O errors.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Error indicating the database pool was accessed before initialization.
    #[error("Database not initialized")]
    NotInitialized,

    /// Catch-all variant for general errors.
    #[error("{0}")]
    Other(String),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

// Implement From<String> to easily convert string errors
impl From<String> for AppError {
    fn from(err: String) -> Self {
        AppError::Other(err)
    }
}
