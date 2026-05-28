//! # Retriever — Distribute Pipeline
//!
//! Finds the most semantically similar existing notes for a given topic chunk.
//!
//! ## Algorithm
//!
//! 1. Embed the chunk text via the LLM service.
//! 2. Run a vector search against all `level=0` fragments (section-level chunks).
//! 3. Rerank the top-10 hits using the scoring reranker to surface the most relevant candidates.
//! 4. Group results by note, keeping only the best (highest-scoring) fragment per note.
//! 5. Return the top-3 notes as [`CandidateNote`]s.
//!
//! ## Why top-3?
//!
//! The downstream LLM prompt receives all candidates and picks the best action
//! (`Append`, `CreateChild`, or `Skip`). Three candidates is sufficient context
//! without overloading the prompt context window.

use crate::error::AppError;
use crate::domain::distribute::CandidateNote;
use crate::ai::LlmService;
use crate::DbState;
use crate::retrieval::types::{SearchResult, FragmentRef};
use std::sync::Arc;

/// Abstraction over candidate-retrieval strategies.
/// Allows the distribute pipeline to be tested with mock retrievers.
#[allow(async_fn_in_trait)]
pub trait Retriever: Send + Sync {
    /// Finds existing notes semantically similar to `text`.
    async fn retrieve_candidates(
        &self,
        state: &DbState,
        text: &str,
        llm_service: &Arc<LlmService>,
    ) -> Result<Vec<CandidateNote>, AppError>;
}

/// Production implementation: embeds the chunk and runs a cosine vector search.
pub struct VectorRetriever;

impl VectorRetriever {
    pub fn new() -> Self {
        Self
    }
}

impl Retriever for VectorRetriever {
    async fn retrieve_candidates(
        &self,
        state: &DbState,
        text: &str,
        llm_service: &Arc<LlmService>,
    ) -> Result<Vec<CandidateNote>, AppError> {
        // 1. Embed the chunk — fall back to zero vector on failure so the pipeline continues.
        let embedding = match llm_service.generate_embeddings(vec![text.to_string()]).await {
            Ok(embs) => {
                if embs.is_empty() {
                    vec![0.0f32; 1536] // Zero vector — will return low-quality candidates
                } else {
                    embs[0].clone()
                }
            }
            Err(_) => {
                vec![0.0f32; 1536]
            }
        };

        // 2. Raw bytes for the SQLite vector_search query
        let embedding_bytes = bytemuck::cast_slice::<f32, u8>(&embedding);
        let scored_hits = state.with_conn(|conn| {
            // level=0 → search section-level chunks (not leaf fragments)
            crate::db::repos::fragments::vector_search(conn, embedding_bytes, Some(0), 10)
        })?;

        // Keep a map of note_id → title for result reconstruction
        let mut title_map = std::collections::HashMap::new();
        for hit in &scored_hits {
            title_map.insert(hit.hit.note_id.0, hit.hit.note_title.clone().unwrap_or_else(|| "Untitled".to_string()));
        }

        // 3. Convert raw hits to SearchResult for the scoring reranker
        let mut search_results: Vec<SearchResult> = scored_hits.into_iter().map(|hit| {
            SearchResult {
                fragment_ref: FragmentRef {
                    note_id: hit.hit.note_id,
                    fragment_index: hit.hit.fragment_index,
                },
                score: hit.score,
                text: Some(hit.hit.text.clone()),
            }
        }).collect();

        // 4. LLM scoring rerank — improves ranking precision over raw cosine distance
        crate::retrieval::rerankers::scoring::rerank_scoring(llm_service, text, &mut search_results, 10.0).await;

        // 5. Group by note, keeping best fragment score per note
        let mut candidates_map: std::collections::HashMap<i64, (String, f32)> = std::collections::HashMap::new();
        for res in search_results {
            let note_id = res.fragment_ref.note_id.0;
            let title = title_map.get(&note_id).cloned().unwrap_or_else(|| "Untitled".to_string());
            let sim = res.score;
            candidates_map.entry(note_id)
                .and_modify(|existing| {
                    if sim > existing.1 {
                        existing.1 = sim; // Keep highest score across fragments of the same note
                    }
                })
                .or_insert((title, sim));
        }

        // 6. Sort and truncate to top-3 candidate notes
        let mut candidates: Vec<CandidateNote> = candidates_map.into_iter()
            .map(|(note_id, (title, similarity))| CandidateNote { note_id, title, similarity })
            .collect();
        candidates.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap_or(std::cmp::Ordering::Equal));
        candidates.truncate(3);

        Ok(candidates)
    }
}
