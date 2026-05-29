use crate::DbState;
use crate::AppError;
use rusqlite::params;
use std::future::Future;

/// Fetches a cached LLM response from the `llm_cache` table.
pub fn get_cached(
    state: &DbState,
    query: &str,
    model_id: &str,
    cache_type: &str,
    target_lang: &str,
) -> Option<String> {
    state.with_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT cached_response FROM llm_cache \
             WHERE query = ? AND model_id = ? AND cache_type = ? AND target_lang = ?"
        ).map_err(AppError::from)?;
        let mut rows = stmt.query(params![query, model_id, cache_type, target_lang]).map_err(AppError::from)?;
        if let Some(row) = rows.next().map_err(AppError::from)? {
            let text: String = row.get(0).map_err(AppError::from)?;
            Ok(Some(text))
        } else {
            Ok(None)
        }
    }).unwrap_or(None)
}

/// Stores an LLM response in the `llm_cache` table.
pub fn put_cached(
    state: &DbState,
    query: &str,
    model_id: &str,
    cache_type: &str,
    target_lang: &str,
    response: &str,
) {
    let _ = state.with_conn(|conn| {
        conn.execute(
            "INSERT OR IGNORE INTO llm_cache (query, model_id, cache_type, target_lang, cached_response) \
             VALUES (?, ?, ?, ?, ?)",
            params![query, model_id, cache_type, target_lang, response],
        ).map_err(AppError::from)?;
        Ok(())
    });
}

/// Helper that checks the cache first, and if missing, runs the future and caches the response.
pub async fn cached_or_run<F>(
    state: &DbState,
    query: &str,
    model_id: &str,
    cache_type: &str,
    target_lang: &str,
    f: F,
) -> Option<String>
where
    F: Future<Output = Option<String>>,
{
    if let Some(cached) = get_cached(state, query, model_id, cache_type, target_lang) {
        return Some(cached);
    }
    let res = f.await;
    if let Some(ref response) = res {
        put_cached(state, query, model_id, cache_type, target_lang, response);
    }
    res
}
