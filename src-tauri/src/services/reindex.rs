//! # Reindex Service
//!
//! Detects embedding-model drift and re-queues affected notes.
//! Triggered manually via CLI or automatically before retrieval.

use rusqlite::params;
use crate::db::state::DbState;

#[derive(Debug, Clone)]
pub struct StaleReport {
    pub current_model: String,
    pub current_dim: i64,
    pub stale_notes: Vec<(i64, Option<String>, Option<i64>)>, // (note_id, old_model, old_dim)
}

/// Finds notes whose `embedding_model` or `embedding_dimension`
/// differs from the supplied current values.
pub fn find_stale_notes(
    state: &DbState,
    current_model: &str,
    current_dim: i64,
) -> Result<StaleReport, String> {
    let pool_guard = state.pool.read();
    let pool = pool_guard.as_ref().ok_or("db not initialized")?;
    let conn = pool.get().map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare(
        "SELECT note_id, embedding_model, embedding_dimension
         FROM notes
         WHERE indexing_status = 'indexed'
           AND (embedding_model IS NULL
                OR embedding_model != ?1
                OR embedding_dimension IS NULL
                OR embedding_dimension != ?2)",
    ).map_err(|e| e.to_string())?;

    let rows = stmt.query_map(params![current_model, current_dim], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, Option<i64>>(2)?))
    }).map_err(|e| e.to_string())?;

    let stale: Vec<_> = rows.filter_map(Result::ok).collect();

    Ok(StaleReport {
        current_model: current_model.to_string(),
        current_dim,
        stale_notes: stale,
    })
}

/// Marks all chunks of stale notes as `embedding_needs_update = 1`
/// and resets the note's `indexing_status = 'pending'`.
/// Returns the number of notes affected.
pub fn mark_stale_for_model_change(
    state: &DbState,
    current_model: &str,
    current_dim: i64,
) -> Result<usize, String> {
    let report = find_stale_notes(state, current_model, current_dim)?;
    if report.stale_notes.is_empty() {
        return Ok(0);
    }

    let pool_guard = state.pool.read();
    let pool = pool_guard.as_ref().ok_or("db not initialized")?;
    let mut conn = pool.get().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    for (note_id, _, _) in &report.stale_notes {
        tx.execute(
            "UPDATE chunks SET embedding = NULL
             WHERE note_id = ?1",
            params![note_id],
        ).map_err(|e| e.to_string())?;
        tx.execute(
            "UPDATE notes SET indexing_status = 'pending', indexed_at = NULL
             WHERE note_id = ?1",
            params![note_id],
        ).map_err(|e| e.to_string())?;
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(report.stale_notes.len())
}
