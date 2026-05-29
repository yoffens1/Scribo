use crate::retrieval::types::{SearchResult, FragmentRef};
use crate::db::repos::fragments;

/// Helper function to prefetch keyword and vector hits for a given query.
pub fn prefetch_hits(
    conn: &rusqlite::Connection,
    query: &str,
    embedding: Option<&[f32]>,
) -> (Vec<SearchResult>, Vec<SearchResult>) {
    let keyword_hits = match fragments::search(conn, query, crate::constants::FUSION_CANDIDATES as i64) {
        Ok(hits) => hits.into_iter().map(|h| SearchResult {
            fragment_ref: FragmentRef {
                note_id: h.hit.note_id,
                fragment_index: h.hit.fragment_index,
            },
            score: h.score,
            text: Some(h.hit.text),
            note_title: h.hit.note_title,
            debug: None,
        }).collect(),
        Err(e) => {
            tracing::warn!("Keyword search error: {}", e);
            Vec::new()
        }
    };

    let vector_hits = if let Some(emb) = embedding {
        let bytes = bytemuck::cast_slice::<f32, u8>(emb).to_vec();
        match fragments::vector_search(conn, &bytes, Some(1), crate::constants::FUSION_CANDIDATES, crate::ai::embedding::CURRENT_EMBEDDING_MODEL, "1") {
            Ok(hits) => hits.into_iter().map(|h| SearchResult {
                fragment_ref: FragmentRef {
                    note_id: h.hit.note_id,
                    fragment_index: h.hit.fragment_index,
                },
                score: h.score,
                text: Some(h.hit.text),
                note_title: h.hit.note_title,
                debug: None,
            }).collect(),
            Err(e) => {
                tracing::warn!("Vector search error: {}", e);
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };

    (keyword_hits, vector_hits)
}
