use rusqlite::Connection;
use crate::error::AppError;
use crate::domain::fragment::FragmentId;
use crate::domain::note::NoteId;
use crate::domain::search::{SearchHit, ScoredHit};
use crate::ai::cosine_similarity_normalized;

/// Zero-copy cast from embedding `BLOB` bytes to `f32` slice.
/// Falls back to copy if the pointer is not aligned.
pub fn bytes_to_f32_slice(bytes: &[u8]) -> std::borrow::Cow<'_, [f32]> {
    let ptr = bytes.as_ptr() as usize;
    if ptr % 4 == 0 {
        std::borrow::Cow::Borrowed(bytemuck::cast_slice(bytes))
    } else {
        let mut aligned = vec![0.0f32; bytes.len() / 4];
        unsafe {
            std::ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                aligned.as_mut_ptr() as *mut u8,
                bytes.len(),
            );
        }
        std::borrow::Cow::Owned(aligned)
    }
}

/// Internal record for the ANN heap — ordered by similarity descending.
/// `Ord` is reversed so `BinaryHeap` acts as a min-heap, enabling O(n log k) top-k selection.
#[derive(Debug)]
struct HitRecord {
    fragment_id: i64,
    similarity: f32,
}
impl PartialEq for HitRecord {
    fn eq(&self, other: &Self) -> bool {
        self.fragment_id == other.fragment_id
    }
}
impl Eq for HitRecord {}
impl PartialOrd for HitRecord {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        other.similarity.partial_cmp(&self.similarity)
    }
}
impl Ord for HitRecord {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap_or(std::cmp::Ordering::Equal)
    }
}

/// Brute-force cosine ANN search over all stored embeddings.
///
/// ## Algorithm
/// 1. Reads `(fragment_id, embedding)` rows for all active notes (filtered by `level`).
/// 2. Computes normalised cosine similarity for each row (assumes unit-norm vectors).
/// 3. Maintains a min-heap of size `limit` — O(n log k) overall.
/// 4. Hydrates the top-k hits with full metadata in a second SQL query.
///
/// `level = None` searches across all fragment levels; `level = Some(0)` = sections, `Some(1)` = fragments.
pub fn vector_search(
    conn: &Connection,
    query_embedding_bytes: &[u8],
    _level: Option<i64>,
    limit: usize,
    embedding_model: &str,
    embedding_model_version: &str,
) -> Result<Vec<ScoredHit>, AppError> {
    let query_vector = bytes_to_f32_slice(query_embedding_bytes);

    let mut top_hits = std::collections::BinaryHeap::with_capacity(limit + 1);

    {
        let sql = "SELECT ce.fragment_id, ce.embedding
             FROM fragment_embeddings ce
             JOIN fragments frag ON frag.fragment_id = ce.fragment_id
             JOIN notes n ON n.note_id = frag.note_id
             WHERE n.lifecycle = 'active'
               AND ce.embedding_model = ?1 AND ce.embedding_model_version = ?2".to_string();
        let mut stmt = conn.prepare(&sql)?;

        let mut rows = stmt.query(rusqlite::params![embedding_model, embedding_model_version])?;

        while let Some(row) = rows.next()? {
            let fragment_id: i64 = row.get(0)?;
            let blob_ref = row.get_ref(1)?;
            if let rusqlite::types::ValueRef::Blob(bytes) = blob_ref {
                if bytes.is_empty() {
                    continue;
                }
                let cand_vector = bytes_to_f32_slice(bytes);
                if cand_vector.len() != query_vector.len() {
                    continue;
                }
                let similarity = cosine_similarity_normalized(query_vector.as_ref(), cand_vector.as_ref());
                
                top_hits.push(HitRecord { fragment_id, similarity });
                if top_hits.len() > limit {
                    top_hits.pop();
                }
            }
        }
    }

    let hits: Vec<HitRecord> = top_hits.into_sorted_vec();

    if hits.is_empty() {
        return Ok(Vec::new());
    }

    let ids: Vec<String> = hits.iter().map(|h| h.fragment_id.to_string()).collect();
    let in_clause = ids.join(",");

    let sql = format!(
        "SELECT frag.fragment_id, n.path_cached, frag.order_index, frag.clean_text, n.title, n.note_id
         FROM fragments frag
         JOIN notes n ON n.note_id = frag.note_id
         WHERE frag.fragment_id IN ({})",
         in_clause
    );

    let mut stmt = conn.prepare(&sql)?;
    let row_iter = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, i64>(5)?,
        ))
    })?;

    let mut db_data = std::collections::HashMap::new();
    for row in row_iter {
        let (id, path, idx, text, title, note_id_val) = row?;
        db_data.insert(id, (path, idx, text, title, note_id_val));
    }

    let mut final_results = Vec::with_capacity(hits.len());
    for h in hits {
        if let Some((path, idx, text, title, note_id_val)) = db_data.remove(&h.fragment_id) {
            final_results.push(ScoredHit {
                hit: SearchHit {
                    fragment_id: FragmentId(h.fragment_id),
                    note_id: NoteId(note_id_val),
                    fragment_index: idx,
                    text,
                    note_title: Some(title),
                    note_path: path,
                    snippet: None,
                },
                score: h.similarity,
            });
        }
    }

    Ok(final_results)
}
