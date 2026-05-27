use crate::error::AppError;
use crate::domain::distribute::CandidateNote;
use crate::ai::LlmService;
use crate::DbState;

#[allow(async_fn_in_trait)]
pub trait Retriever: Send + Sync {
    async fn retrieve_candidates(
        &self,
        state: &DbState,
        text: &str,
        llm_service: &LlmService,
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
        llm_service: &LlmService,
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
            crate::db::repos::fragments::vector_search(conn, embedding_bytes, 5)
        })?;

        let mut candidates_map: std::collections::HashMap<i64, (String, f32)> = std::collections::HashMap::new();
        for hit in scored_hits {
            let note_id = hit.hit.note_id.0;
            let title = hit.hit.note_title.clone().unwrap_or_else(|| "Untitled".to_string());
            let sim = hit.score;
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
