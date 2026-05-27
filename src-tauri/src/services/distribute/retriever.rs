use crate::error::AppError;
use crate::domain::distribute::CandidateNote;
use crate::ai::LlmService;
use crate::DbState;
use crate::retrieval::types::{SearchResult, FragmentRef};
use std::sync::Arc;

#[allow(async_fn_in_trait)]
pub trait Retriever: Send + Sync {
    async fn retrieve_candidates(
        &self,
        state: &DbState,
        text: &str,
        llm_service: &Arc<LlmService>,
    ) -> Result<Vec<CandidateNote>, AppError>;
}

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
        let embedding = match llm_service.generate_embeddings(vec![text.to_string()]).await {
            Ok(embs) => {
                if embs.is_empty() {
                    vec![0.0f32; 1536]
                } else {
                    embs[0].clone()
                }
            }
            Err(_) => {
                vec![0.0f32; 1536]
            }
        };

        let embedding_bytes = bytemuck::cast_slice::<f32, u8>(&embedding);
        let scored_hits = state.with_conn(|conn| {
            crate::db::repos::fragments::vector_search(conn, embedding_bytes, 10)
        })?;

        // Keep a map of note_id -> title for reconstruction
        let mut title_map = std::collections::HashMap::new();
        for hit in &scored_hits {
            title_map.insert(hit.hit.note_id.0, hit.hit.note_title.clone().unwrap_or_else(|| "Untitled".to_string()));
        }

        // Convert ScoredHits to SearchResults for the reranker
        let mut search_results: Vec<SearchResult> = scored_hits.into_iter().map(|hit| {
            SearchResult {
                fragment_ref: FragmentRef {
                    note_id: hit.hit.note_id,
                    fragment_index: hit.hit.fragment_index as usize,
                },
                score: hit.score,
                text: Some(hit.hit.text.clone()),
            }
        }).collect();

        // Rerank using retrieval scoring reranker
        crate::retrieval::rerankers::scoring::rerank_scoring(llm_service, text, &mut search_results).await;

        let mut candidates_map: std::collections::HashMap<i64, (String, f32)> = std::collections::HashMap::new();
        for res in search_results {
            let note_id = res.fragment_ref.note_id.0;
            let title = title_map.get(&note_id).cloned().unwrap_or_else(|| "Untitled".to_string());
            let sim = res.score;
            candidates_map.entry(note_id)
                .and_modify(|existing| {
                    if sim > existing.1 {
                        existing.1 = sim;
                    }
                })
                .or_insert((title, sim));
        }
        
        let mut candidates: Vec<CandidateNote> = candidates_map.into_iter()
            .map(|(note_id, (title, similarity))| CandidateNote { note_id, title, similarity })
            .collect();
        candidates.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap_or(std::cmp::Ordering::Equal));
        candidates.truncate(3);

        Ok(candidates)
    }
}
