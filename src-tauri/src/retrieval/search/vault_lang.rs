//! # Vault Language Detection
//!
//! Detects and caches the dominant language of the vault by sampling stored fragment text.

use crate::DbState;
use crate::db::repos::fragments;
use crate::lang::pick_dominant_language;

/// Detects and returns the dominant language of the vault.
/// Scans the first N sample chunks, detects each fragment's language,
/// and caches the result in the `DbState` for subsequent calls.
pub fn get_vault_language(state: &DbState) -> String {
    if let Some(cached) = state.cached_vault_lang.read().as_ref() {
        return cached.clone();
    }

    let fragments = state.with_conn(|conn| {
        fragments::get_sample_texts(conn, crate::constants::VAULT_LANG_SAMPLE_SIZE as i64)
    }).unwrap_or_default();

    let best_lang = pick_dominant_language(&fragments);

    *state.cached_vault_lang.write() = Some(best_lang.clone());
    best_lang
}
