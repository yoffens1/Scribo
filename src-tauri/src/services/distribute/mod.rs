pub mod chunker;
pub mod classifier;
pub mod retriever;
pub mod apply;

pub use chunker::{Chunker, RuleChunker, split_into_topics, parse_raw_blocks};
pub use classifier::{Classifier, HeuristicClassifier, apply_heuristic_linking};
pub use retriever::{Retriever, VectorRetriever};
pub use apply::apply_distribution;

use crate::error::AppError;
use crate::ai::LlmService;
use crate::domain::distribute::{DraftDistributionPlan, ChunkDistributionPlan, LlmRecommendation, extract_json_payload};

pub async fn analyze_draft_for_distribution(
    state: &crate::DbState,
    draft_id: i64,
    llm_service: &LlmService,
) -> Result<DraftDistributionPlan, AppError> {
    let note = state.with_conn(|conn| {
        crate::db::repos::notes::get_by_id(conn, draft_id)
    })?.ok_or_else(|| AppError::Other(format!("Draft note not found: {}", draft_id)))?;

    let chunker = RuleChunker::new(800);
    let chunks = chunker.chunk(&note.content);
    let mut chunk_plans = Vec::new();

    let retriever = VectorRetriever::new();

    for (idx, chunk) in chunks.into_iter().enumerate() {
        let candidates = retriever.retrieve_candidates(state, &chunk.text, llm_service).await?;

        let candidates_str = if candidates.is_empty() {
            "None".to_string()
        } else {
            candidates.iter()
                .map(|c| format!("ID: {}, Title: \"{}\" (Similarity: {:.4})", c.note_id, c.title, c.similarity))
                .collect::<Vec<_>>()
                .join("\n")
        };
        
        let prompt = crate::ai::prompts::build_distribute_prompt(&chunk.text, &chunk.suggested_title, &candidates_str);
        
        let response = llm_service.generate_messages(vec![crate::ai::types::Message {
            role: "user".into(),
            content: prompt,
        }]).await;

        let recommendation = match response {
            Ok(res) => {
                if let Some(json_str) = extract_json_payload(&res.text) {
                    match serde_json::from_str::<LlmRecommendation>(&json_str) {
                        Ok(rec) => rec,
                        Err(e) => LlmRecommendation {
                            action: "skip".to_string(),
                            target_note_id: None,
                            new_note_title: None,
                            parent_note_id: None,
                            reason: format!("Failed to parse LLM response: {}. Raw: {}", e, res.text),
                        }
                    }
                } else {
                    LlmRecommendation {
                        action: "skip".to_string(),
                        target_note_id: None,
                        new_note_title: None,
                        parent_note_id: None,
                        reason: format!("No JSON object found in LLM response. Raw: {}", res.text),
                    }
                }
            }
            Err(e) => LlmRecommendation {
                action: "skip".to_string(),
                target_note_id: None,
                new_note_title: None,
                parent_note_id: None,
                reason: format!("LLM generation failed: {}", e),
            }
        };

        chunk_plans.push(ChunkDistributionPlan {
            chunk_index: idx,
            text: chunk.text,
            suggested_title: chunk.suggested_title,
            candidates,
            recommendation,
        });
    }

    let mut plan = DraftDistributionPlan {
        draft_id,
        chunks: chunk_plans,
    };
    
    let classifier = HeuristicClassifier::new();
    classifier.classify(&mut plan.chunks);

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
