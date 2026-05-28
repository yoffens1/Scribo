//! # Timestamp Utility
//!
//! Provides the canonical wall-clock timestamp used throughout the database layer.
//! All `created_at` / `updated_at` / `reviewed_at` columns are stored as **Unix seconds (i64)**.
//!
//! Using a single function instead of inline `SystemTime::now()` calls ensures:
//! - A consistent time resolution across all DB writes in one request.
//! - Easy mocking in tests (replace with a fixed value if needed).

use std::time::{SystemTime, UNIX_EPOCH};

/// Returns the current UTC time as a Unix timestamp in **whole seconds**.
/// Panics are impossible — `unwrap_or_default()` returns 0 if the system clock
/// is before the Unix epoch (impossible in practice).
pub fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
