pub mod chunker;
pub mod classifier;
pub mod retriever;
pub mod apply;
pub mod refresh_cards;

pub use chunker::{Chunker, RuleChunker, SemanticChunker, split_into_topics, parse_raw_blocks};
pub use classifier::{Classifier, HeuristicClassifier, apply_heuristic_linking};
pub use retriever::{Retriever, VectorRetriever};
pub use apply::apply_distribution;
pub use refresh_cards::refresh_stale_cards_for_notes;

use crate::error::AppError;
use crate::ai::{LlmService, extract_json_payload};
use crate::domain::distribute::{DraftDistributionPlan, ChunkDistributionPlan, LlmRecommendation, DistributeAction};
use std::sync::Arc;

pub async fn analyze_draft_for_distribution(
    state: &crate::DbState,
    draft_id: i64,
    llm_service: &Arc<LlmService>,
) -> Result<DraftDistributionPlan, AppError> {
    let note = state.with_conn(|conn| {
        crate::db::repos::notes::get_by_id(conn, draft_id)
    })?.ok_or_else(|| AppError::Other(format!("Draft note not found: {}", draft_id)))?;

    // 1. Semantic Chunker
    let chunker = SemanticChunker::new(800, 0.7);
    let chunks = chunker.chunk(&note.content, llm_service).await;
    if chunks.is_empty() {
        return Ok(DraftDistributionPlan {
            draft_id,
            chunks: Vec::new(),
        });
    }

    // 2. Parallel Retrieval
    let retriever = VectorRetriever::new();
    let mut candidate_futures = Vec::new();
    for chunk in &chunks {
        candidate_futures.push(retriever.retrieve_candidates(state, &chunk.text, llm_service));
    }
    let candidates_results = futures::future::join_all(candidate_futures).await;

    // 3. Prepare prompt input
    let mut prompt_inputs = Vec::new();
    let mut candidates_list = Vec::new();
    for (idx, chunk) in chunks.iter().enumerate() {
        let candidates = match &candidates_results[idx] {
            Ok(cands) => cands.clone(),
            Err(_) => Vec::new(),
        };

        let candidates_str = if candidates.is_empty() {
            "None".to_string()
        } else {
            candidates.iter()
                .map(|c| format!("ID: {}, Title: \"{}\" (Similarity: {:.4})", c.note_id, c.title, c.similarity))
                .collect::<Vec<_>>()
                .join("\n")
        };

        prompt_inputs.push((chunk.text.as_str(), chunk.suggested_title.as_str(), candidates_str));
        candidates_list.push(candidates);
    }

    let prompt_inputs_borrowed: Vec<(&str, &str, &str)> = prompt_inputs.iter()
        .map(|(t, s, c)| (*t, *s, c.as_str()))
        .collect();

    // 4. Batch prompt
    let prompt = crate::ai::prompts::build_batch_distribute_prompt(&prompt_inputs_borrowed);
    
    let response = llm_service.generate_messages(vec![crate::ai::types::Message {
        role: "user".into(),
        content: prompt,
    }]).await;

    let mut recommendations = Vec::new();
    if let Ok(res) = response {
        if let Some(json_str) = extract_json_payload(&res.text) {
            match serde_json::from_str::<Vec<LlmRecommendation>>(&json_str) {
                Ok(recs) => {
                    recommendations = recs;
                }
                Err(e) => {
                    for _ in 0..chunks.len() {
                        recommendations.push(LlmRecommendation {
                            action: DistributeAction::Skip { reason: format!("Failed to parse batch JSON: {}. Raw: {}", e, res.text) },
                            tags: None,
                            confidence: None,
                            reason: "Failed to parse batch JSON".to_string(),
                        });
                    }
                }
            }
        }
    }

    while recommendations.len() < chunks.len() {
        recommendations.push(LlmRecommendation {
            action: DistributeAction::Skip { reason: "LLM returned incomplete recommendations".to_string() },
            tags: None,
            confidence: None,
            reason: "LLM returned incomplete recommendations".to_string(),
        });
    }

    let mut chunk_plans = Vec::new();
    for (idx, chunk) in chunks.into_iter().enumerate() {
        chunk_plans.push(ChunkDistributionPlan {
            chunk_index: idx,
            text: chunk.text,
            suggested_title: chunk.suggested_title,
            candidates: candidates_list[idx].clone(),
            recommendation: recommendations[idx].clone(),
        });
    }

    let plan = DraftDistributionPlan {
        draft_id,
        chunks: chunk_plans,
    };

    // Save distribution run audit log
    let plan_json = serde_json::to_string(&plan).unwrap_or_default();
    state.with_conn(|conn| {
        conn.execute(
            "INSERT INTO distribution_runs (draft_id, plan_json, generator_version, status, created_at)
             VALUES (?, ?, 'v1', 'analyzed', strftime('%s','now'))",
            rusqlite::params![draft_id, plan_json],
        )?;
        Ok(())
    }).map_err(|e: AppError| e)?;

    Ok(plan)
}
