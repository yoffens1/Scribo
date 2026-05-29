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

// ─── Byte-range alignment helpers ────────────────────────────────────────────

/// Rounds `idx` down to the nearest valid UTF-8 char boundary in `s`.
fn align_to_char_boundary_floor(s: &str, mut idx: usize) -> usize {
    if idx > s.len() {
        idx = s.len();
    }
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

/// Rounds `idx` up to the nearest valid UTF-8 char boundary in `s`.
fn align_to_char_boundary_ceil(s: &str, mut idx: usize) -> usize {
    if idx > s.len() {
        return s.len();
    }
    while idx < s.len() && !s.is_char_boundary(idx) {
        idx += 1;
    }
    idx
}

/// Locates the byte range of `text_raw` inside `content`, starting the search from `last_index`.
///
/// Falls back gracefully if an exact match is not found:
/// 1. Tries to match the first line of `text_raw` as a prefix anchor.
/// 2. If that also fails, returns a best-effort range starting at `last_index`.
///
/// All returned indices are aligned to UTF-8 char boundaries.
fn find_safe_offsets(content: &str, text_raw: &str, last_index: usize) -> (usize, usize) {
    if let Some(idx) = content[last_index..].find(text_raw) {
        return (last_index + idx, last_index + idx + text_raw.len());
    }

    // Fallback 1: match first line as anchor
    let prefix = text_raw.lines().next().unwrap_or("").trim();
    if !prefix.is_empty() {
        if let Some(idx) = content[last_index..].find(prefix) {
            let start = last_index + idx;
            let mut end = start + text_raw.len();
            if end > content.len() {
                end = content.len();
            }
            let start_aligned = align_to_char_boundary_floor(content, start);
            let end_aligned = align_to_char_boundary_ceil(content, end);
            return (start_aligned, end_aligned);
        }
    }

    // Fallback 2: best-effort range from last_index
    let start = align_to_char_boundary_floor(content, last_index);
    let mut end = start + text_raw.len();
    if end > content.len() {
        end = content.len();
    }
    let end_aligned = align_to_char_boundary_ceil(content, end);
    (start, end_aligned)
}

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
/// 2. Load existing sections and fragments for comparison.
/// 3. Run `fragment_paired` to get new `(generation, embedding)` pairs.
/// 4. Compute byte offsets of each fragment in the original markdown.
/// 5. Loop over `max(new, old)` indices:
///    - If a new fragment exists: upsert section + fragment by hash comparison.
///    - If only an old row exists (count shrank): delete the orphaned row.
/// 6. Mark the note as `indexed` and commit.
pub fn persist_indexed_file(
    conn: &mut Connection,
    payload: IndexingPayload,
) -> Result<i64, AppError> {
    let note_id = payload.note_id;

    // 1. Fetch note record
    let note = crate::db::repos::notes::get_by_id(conn, note_id)?
        .ok_or_else(|| AppError::Other(format!("Note not found: {}", note_id)))?;

    // 2. Fetch existing fragments & sections for hash-based diff
    let old_fragments = crate::db::repos::fragments::list_by_note(conn, note_id, payload.embedding_model)
        .map_err(|e| AppError::Other(e.to_string()))?;
    let old_sections = crate::db::repos::sections::list_by_note(conn, note_id)
        .map_err(|e| AppError::Other(e.to_string()))?;

    // 3. Run the fragmenter to obtain (generation_text, embedding_text) pairs
    let options = FragmentOptions::default();
    let chunk_result = fragment_paired(note.content.clone(), &options);

    // 4. Calculate byte offsets of each fragment in the original markdown
    let mut last_index = 0;
    let mut section_offsets = Vec::new();
    for pair in &chunk_result.pairs {
        let text_raw = &pair.generation;
        let (start, end) = find_safe_offsets(&note.content, text_raw, last_index);
        section_offsets.push((start as i64, end as i64));
        last_index = end;
    }

    let max_len = std::cmp::max(
        chunk_result.pairs.len(),
        std::cmp::max(old_sections.len(), old_fragments.len())
    );

    let tx = conn.transaction().map_err(|e| AppError::Other(e.to_string()))?;

    // 5. Upsert or delete each slot
    for i in 0..max_len {
        if i < chunk_result.pairs.len() {
            let pair = &chunk_result.pairs[i];

            // Section (level=0): stores the raw/generation text with its offset
            let text_raw = &pair.generation;
            let source_hash_raw = content_hash(text_raw);
            let text_clean = &pair.embedding;
            let source_hash_clean = content_hash(text_clean);
            let (offset_start, offset_end) = section_offsets[i];

            let section_chunk_id = if i < old_sections.len() {
                let old_sec = &old_sections[i];
                // Only write if content or offset changed
                if old_sec.raw_hash != source_hash_raw
                    || old_sec.content_offset_start != offset_start
                    || old_sec.content_offset_end != offset_end
                {
                    let (heading, level) = extract_heading_from_markdown(text_raw);
                    crate::db::repos::sections::update(
                        &tx,
                        old_sec.id.0,
                        text_raw,
                        heading.as_deref(),
                        level,
                        &source_hash_raw,
                        &source_hash_clean,
                        offset_start,
                        offset_end,
                    ).map_err(|e| AppError::Other(e.to_string()))?;

                    // Mark any SRS cards derived from this section as stale
                    crate::db::repos::cards::mark_stale_for_section(&tx, old_sec.id.0)
                        .map_err(|e| AppError::Other(e.to_string()))?;
                }
                old_sec.id.0
            } else {
                let (heading, level) = extract_heading_from_markdown(text_raw);
                crate::db::repos::sections::insert_single(
                    &tx,
                    note_id,
                    i as i64,
                    text_raw,
                    heading.as_deref(),
                    level,
                    &source_hash_raw,
                    &source_hash_clean,
                    offset_start,
                    offset_end,
                ).map_err(|e| AppError::Other(e.to_string()))?
            };

            // Fragment (level=1): stores the embedding-clean text (no embeddings yet)
            if i < old_fragments.len() {
                let old_frag = &old_fragments[i];
                if old_frag.clean_hash != source_hash_clean {
                    // Hash changed — update the row and clear the stale embedding
                    crate::db::repos::fragments::update(
                        &tx,
                        note_id,
                        i as i64,
                        text_clean,
                        &source_hash_clean,
                        true, // embedding_needs_update = true
                        Some(section_chunk_id),
                    ).map_err(|e| AppError::Other(e.to_string()))?;
                }
            } else {
                crate::db::repos::fragments::insert_single(
                    &tx,
                    note_id,
                    i as i64,
                    text_clean,
                    &source_hash_clean,
                    None,   // embedding blob — written later by the embedder
                    &[],
                    Some(section_chunk_id),
                ).map_err(|e| AppError::Other(e.to_string()))?;
            }
        } else {
            // New fragment count shrank — delete orphaned rows
            if i < old_sections.len() {
                crate::db::repos::sections::delete_by_id(
                    &tx,
                    old_sections[i].id.0,
                ).map_err(|e| AppError::Other(e.to_string()))?;
            }
            if i < old_fragments.len() {
                crate::db::repos::fragments::delete_by_id(
                    &tx,
                    old_fragments[i].id.0,
                ).map_err(|e| AppError::Other(e.to_string()))?;
            }
        }
    }

    // 6. Mark the note as indexed and commit the transaction
    tx.execute(
        "UPDATE notes SET indexing_status = 'indexed', indexing_error = NULL, indexed_at = ?, embedding_model = ?, embedding_dimension = ?, indexing_version = ? WHERE note_id = ?",
        rusqlite::params![crate::db::time::now_seconds(), payload.embedding_model, payload.embedding_dim as i64, payload.indexing_version, note_id],
    ).map_err(|e| AppError::Other(e.to_string()))?;

    tx.commit().map_err(|e| AppError::Other(e.to_string()))?;

    Ok(note_id)
}
