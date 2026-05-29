//! # Candidate Search — Distribute Pipeline
//!
//! Finds the most semantically similar existing notes for a given topic chunk.

use crate::error::AppError;
use crate::domain::distribute::CandidateNote;
use crate::ai::LlmService;
use crate::DbState;
use crate::retrieval::{retrieve, RetrievalConfig, RetrieveOptions};
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

/// Production implementation: calls the unified retrieval query stage.
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
        // 1. Temporarily populate the state's LlmService cache so retrieve() uses it.
        {
            let mut guard = state.llm_service.write();
            *guard = Some((llm_service.config().clone(), Arc::clone(llm_service)));
        }

        // 2. Build pre-configured semantic search config for distribute targets (level=0 section-level chunks).
        let config = RetrievalConfig {
            mode: crate::retrieval::types::RetrievalMode::Embedding,
            vault_lang: None,
            embedding_weight: Some(1.0),
            ai_rerank: Some(crate::retrieval::types::AiRerankConfig {
                enabled: true,
                mode: Some(crate::retrieval::types::RerankMode::Scoring),
                max_candidates: Some(10),
            }),
            pipeline: None,
            tuning: None,
            adaptive_weights: None,
            llm_config: None,
        };
        let options = RetrieveOptions {
            top_k: Some(10),
            target_level: Some(0), // Search section-level chunks, not leaf fragments.
            ..Default::default()
        };

        // 3. Execute unified retrieve pipeline.
        let search_results = retrieve(state, text, None, &config, &options).await?;

        // 4. Extract note_ids and fetch titles from notes table.
        let note_ids: Vec<i64> = search_results.iter().map(|r| r.fragment_ref.note_id.0).collect();
        let title_map = if note_ids.is_empty() {
            std::collections::HashMap::new()
        } else {
            state.with_conn(|conn| {
                let place_holders = vec!["?"; note_ids.len()].join(",");
                let sql = format!("SELECT note_id, title FROM notes WHERE note_id IN ({})", place_holders);
                let mut stmt = conn.prepare(&sql)?;
                let rows = stmt.query_map(rusqlite::params_from_iter(note_ids.iter()), |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                })?;
                let mut map = std::collections::HashMap::new();
                for r in rows {
                    if let Ok((id, title)) = r {
                        map.insert(id, title);
                    }
                }
                Ok(map)
            })?
        };

        // 5. Group by note, keeping highest score (similarity) per note.
        let mut candidates_map: std::collections::HashMap<i64, (String, f32)> = std::collections::HashMap::new();
        for res in search_results {
            let note_id = res.fragment_ref.note_id.0;
            let title = title_map.get(&note_id).cloned().unwrap_or_else(|| "Untitled".to_string());
            let similarity = res.score;
            candidates_map.entry(note_id)
                .and_modify(|existing| {
                    if similarity > existing.1 {
                        existing.1 = similarity;
                    }
                })
                .or_insert((title, similarity));
        }

        // 6. Sort and truncate to top-3 candidate notes.
        let mut candidates: Vec<CandidateNote> = candidates_map.into_iter()
            .map(|(note_id, (title, similarity))| CandidateNote { note_id, title, similarity })
            .collect();
        candidates.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap_or(std::cmp::Ordering::Equal));
        candidates.truncate(3);

        Ok(candidates)
    }
}
