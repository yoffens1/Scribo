use rusqlite::Connection;
use crate::AppError;
use crate::fragmenter::{fragment_paired, FragmentOptions};
use crate::db::hash::content_hash;

fn align_to_char_boundary_floor(s: &str, mut idx: usize) -> usize {
    if idx > s.len() {
        idx = s.len();
    }
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

fn align_to_char_boundary_ceil(s: &str, mut idx: usize) -> usize {
    if idx > s.len() {
        return s.len();
    }
    while idx < s.len() && !s.is_char_boundary(idx) {
        idx += 1;
    }
    idx
}

fn find_safe_offsets(content: &str, text_raw: &str, last_index: usize) -> (usize, usize) {
    if let Some(idx) = content[last_index..].find(text_raw) {
        return (last_index + idx, last_index + idx + text_raw.len());
    }
    
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
    
    let start = align_to_char_boundary_floor(content, last_index);
    let mut end = start + text_raw.len();
    if end > content.len() {
        end = content.len();
    }
    let end_aligned = align_to_char_boundary_ceil(content, end);
    (start, end_aligned)
}

pub struct IndexingPayload<'a> {
    pub note_id: i64,
    pub embedding_model: &'a str,
    pub embedding_dim: u32,
    pub indexing_version: &'a str,
}

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

pub fn persist_indexed_file(
    conn: &mut Connection,
    payload: IndexingPayload,
) -> Result<i64, AppError> {
    let note_id = payload.note_id;

    // 1. Fetch note record
    let note = crate::db::repos::notes::get_by_id(conn, note_id)?
        .ok_or_else(|| AppError::Other(format!("Note not found: {}", note_id)))?;

    // 2. Fetch existing fragments & sections
    let old_fragments = crate::db::repos::fragments::list_by_note(conn, note_id)
        .map_err(|e| AppError::Other(e.to_string()))?;
    let old_sections = crate::db::repos::sections::list_by_note(conn, note_id)
        .map_err(|e| AppError::Other(e.to_string()))?;

    // 3. Chunk the document content
    let options = FragmentOptions::default();
    let chunk_result = fragment_paired(note.content.clone(), &options);

    // 4. Update fragments with diffing logic
    let max_frag_len = std::cmp::max(old_fragments.len(), chunk_result.pairs.len());
    for i in 0..max_frag_len {
        if i < chunk_result.pairs.len() && i < old_fragments.len() {
            let text_clean = &chunk_result.pairs[i].embedding;
            let source_hash = content_hash(text_clean);
            if old_fragments[i].source_hash != source_hash {
                // Fragment changed, update and clear embedding
                crate::db::repos::fragments::update(
                    conn,
                    note_id,
                    i as i64,
                    text_clean,
                    &source_hash,
                    true,
                ).map_err(|e| AppError::Other(e.to_string()))?;
            }
        } else if i < chunk_result.pairs.len() {
            // New fragment, insert
            let text_clean = &chunk_result.pairs[i].embedding;
            let source_hash = content_hash(text_clean);
            crate::db::repos::fragments::insert_single(
                conn,
                note_id,
                i as i64,
                text_clean,
                &source_hash,
                None,
                &[],
            ).map_err(|e| AppError::Other(e.to_string()))?;
        } else {
            // Old fragment removed, delete
            crate::db::repos::fragments::delete_by_id(
                conn,
                old_fragments[i].id.0,
            ).map_err(|e| AppError::Other(e.to_string()))?;
        }
    }

    // 5. Update sections with diffing logic
    let mut last_index = 0;
    let mut section_offsets = Vec::new();
    for pair in &chunk_result.pairs {
        let text_raw = &pair.generation;
        let (start, end) = find_safe_offsets(&note.content, text_raw, last_index);
        section_offsets.push((start as i64, end as i64));
        last_index = end;
    }

    let max_sec_len = std::cmp::max(old_sections.len(), chunk_result.pairs.len());
    let tx = conn.transaction().map_err(|e| AppError::Other(e.to_string()))?;
    
    for i in 0..max_sec_len {
        if i < chunk_result.pairs.len() && i < old_sections.len() {
            let text_raw = &chunk_result.pairs[i].generation;
            let source_hash = content_hash(text_raw);
            let (offset_start, offset_end) = section_offsets[i];
            if old_sections[i].source_hash != source_hash
                || old_sections[i].content_offset_start != offset_start
                || old_sections[i].content_offset_end != offset_end
            {
                let (heading, level) = extract_heading_from_markdown(text_raw);
                // Section changed, update & mark cards stale
                crate::db::repos::sections::update(
                    &tx,
                    old_sections[i].id.0,
                    text_raw,
                    heading.as_deref(),
                    level,
                    &source_hash,
                    offset_start,
                    offset_end,
                ).map_err(|e| AppError::Other(e.to_string()))?;

                crate::db::repos::cards::mark_stale_for_section(&tx, old_sections[i].id.0)
                    .map_err(|e| AppError::Other(e.to_string()))?;
            }
        } else if i < chunk_result.pairs.len() {
            // New section, insert & create card
            let text_raw = &chunk_result.pairs[i].generation;
            let source_hash = content_hash(text_raw);
            let (heading, level) = extract_heading_from_markdown(text_raw);
            let (offset_start, offset_end) = section_offsets[i];
            let _new_sec_id = crate::db::repos::sections::insert_single(
                &tx,
                note_id,
                i as i64,
                text_raw,
                heading.as_deref(),
                level,
                &source_hash,
                offset_start,
                offset_end,
            ).map_err(|e| AppError::Other(e.to_string()))?;

            // Note: cards are no longer auto-created during indexing,
            // they are created/generated manually when the user requests them.
        } else {
            // Old section removed, delete (database cascades will handle cards)
            crate::db::repos::sections::delete_by_id(
                &tx,
                old_sections[i].id.0,
            ).map_err(|e| AppError::Other(e.to_string()))?;
        }
    }

    // 6. Update note metadata and status
    tx.execute(
        "UPDATE notes SET indexing_status = 'indexed', indexing_error = NULL, indexed_at = ?, embedding_model = ?, embedding_dimension = ?, indexing_version = ? WHERE note_id = ?",
        rusqlite::params![crate::db::time::now_seconds(), payload.embedding_model, payload.embedding_dim as i64, payload.indexing_version, note_id],
    ).map_err(|e| AppError::Other(e.to_string()))?;

    tx.commit().map_err(|e| AppError::Other(e.to_string()))?;

    Ok(note_id)
}
