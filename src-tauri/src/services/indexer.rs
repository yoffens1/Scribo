//! # Indexer Service
//!
//! Converts a note's markdown content into indexed fragments persisted in the `chunks` table.
//!
//! ## Responsibilities
//!
//! 1. **Fragment extraction** — runs `fragment_paired` to produce `(embedding_text, generation_text)` pairs.
//! 2. **Offset calculation** — maps each fragment back to its byte range in the original note content.
//! 3. **Upsert logic** — compares content hashes of existing DB rows against newly generated fragments.
//!    Only fragments whose hash changed are updated, avoiding unnecessary writes.
//! 4. **Orphan deletion** — if the new fragment count is smaller than the old one, excess rows are deleted.
//! 5. **Status update** — sets `indexing_status = 'indexed'` on the note after a successful run.
//!
//! ## Relationship to embedding
//!
//! `persist_indexed_file` writes the *text* of each fragment but does **not** compute or store embeddings —
//! that responsibility belongs to the embedding pipeline which runs after this service and updates
//! the `embedding` blob column on each `chunks` row.

use rusqlite::Connection;
use crate::AppError;
use crate::fragmenter::{fragment_paired, FragmentOptions};
use crate::db::hash::content_hash;

// ─── Public API ───────────────────────────────────────────────────────────────

/// Metadata required to index a note.
pub struct IndexingPayload<'a> {
    pub note_id: i64,
    /// Identifier string of the embedding model (e.g. `"nomic-embed-text"`).
    pub embedding_model: &'a str,
    /// Expected vector dimensionality for the model.
    pub embedding_dim: u32,
    /// Semantic version of the indexing pipeline (used for stale-detection).
    pub indexing_version: &'a str,
}

/// Extracts the first markdown heading from `text` and returns `(title, level)`.
/// Returns `(None, None)` if no heading is found.
pub fn extract_heading_from_markdown(text: &str) -> (Option<String>, Option<i64>) {
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            let hash_count = trimmed.chars().take_while(|&c| c == '#').count();
            if hash_count >= 1 && hash_count <= 6 {
                let heading_text = trimmed[hash_count..].trim().to_string();
                return (Some(heading_text), Some(hash_count as i64));
            }
        }
    }
    (None, None)
}

/// Fragments the note, diffs against existing DB rows, and persists the result in a single transaction.
///
/// ## Steps
///
/// 1. Fetch the note record.
/// 2. Load existing level=1 fragments for hash-based diffing.
/// 3. Run `fragment_paired` to get new `(generation, embedding)` pairs.
/// 4. Loop over `max(new, old)` indices:
///    - If a new fragment exists: upsert by hash comparison.
///    - If only an old row exists (count shrank): delete the orphaned row.
/// 5. Mark the note as `indexed` and commit.
pub fn persist_indexed_file(
    conn: &mut Connection,
    payload: IndexingPayload,
) -> Result<i64, AppError> {
    let note_id = payload.note_id;

    // 1. Fetch note record
    let note = crate::db::repos::notes::get_by_id(conn, note_id)?
        .ok_or_else(|| AppError::Other(format!("Note not found: {}", note_id)))?;

    // 2. Fetch existing level=1 fragments for hash-based diff
    let old_fragments = crate::db::repos::fragments::list_by_note(conn, note_id, payload.embedding_model)
        .map_err(|e| AppError::Other(e.to_string()))?;

    // 3. Run the fragmenter to obtain (generation_text, embedding_text) pairs
    let options = FragmentOptions::default();
    let chunk_result = fragment_paired(note.content.clone(), &options);

    let max_len = std::cmp::max(chunk_result.pairs.len(), old_fragments.len());

    let tx = conn.transaction().map_err(|e| AppError::Other(e.to_string()))?;

    // 4. Upsert or delete each slot
    for i in 0..max_len {
        if i < chunk_result.pairs.len() {
            let pair = &chunk_result.pairs[i];
            let text_raw = &pair.generation;
            let source_hash_raw = content_hash(text_raw);
            let text_clean = &pair.embedding;
            let source_hash_clean = content_hash(text_clean);

            if i < old_fragments.len() {
                let old_frag = &old_fragments[i];
                if old_frag.clean_hash != source_hash_clean {
                    // Hash changed — update the row and invalidate the stale embedding
                    crate::db::repos::fragments::update(
                        &tx,
                        note_id,
                        i as i64,
                        text_raw,
                        &source_hash_raw,
                        text_clean,
                        &source_hash_clean,
                        true, // clear_embedding = true
                    ).map_err(|e| AppError::Other(e.to_string()))?;
                }
            } else {
                crate::db::repos::fragments::insert_single(
                    &tx,
                    note_id,
                    i as i64,
                    text_raw,
                    &source_hash_raw,
                    text_clean,
                    &source_hash_clean,
                    &[], // embedding blob — written later by the embedder
                ).map_err(|e| AppError::Other(e.to_string()))?;
            }
        } else {
            // New fragment count shrank — delete orphaned rows
            if i < old_fragments.len() {
                crate::db::repos::fragments::delete_by_id(
                    &tx,
                    old_fragments[i].id.0,
                ).map_err(|e| AppError::Other(e.to_string()))?;
            }
        }
    }

    // 5. Mark the note as indexed and commit the transaction
    tx.execute(
        "UPDATE notes SET indexing_status = 'indexed', indexing_error = NULL, indexed_at = ?, embedding_model = ?, embedding_dimension = ?, indexing_version = ? WHERE note_id = ?",
        rusqlite::params![crate::db::time::now_seconds(), payload.embedding_model, payload.embedding_dim as i64, payload.indexing_version, note_id],
    ).map_err(|e| AppError::Other(e.to_string()))?;

    tx.commit().map_err(|e| AppError::Other(e.to_string()))?;

    Ok(note_id)
}
