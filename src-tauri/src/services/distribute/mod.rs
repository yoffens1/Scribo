mod types;
mod parser;
mod heuristic;
mod tests;

pub use types::{
    TopicChunk, RawBlock, CandidateNote, LlmRecommendation,
    ChunkDistributionPlan, DraftDistributionPlan,
};
pub use parser::{parse_raw_blocks, split_into_topics};

use rusqlite::Connection;
use crate::error::AppError;
use crate::ai::LlmService;
use crate::domain::note::{NoteId, NewNote};
use types::extract_json_payload;
use heuristic::apply_heuristic_linking;

pub async fn analyze_draft_for_distribution(
    state: &crate::DbState,
    draft_id: i64,
    llm_service: &LlmService,
) -> Result<DraftDistributionPlan, AppError> {
    let note = state.with_conn(|conn| {
        crate::db::repos::notes::get_by_id(conn, draft_id)
    })?.ok_or_else(|| AppError::Other(format!("Draft note not found: {}", draft_id)))?;

    let chunks = split_into_topics(&note.content, 800);
    let mut chunk_plans = Vec::new();

    for (idx, chunk) in chunks.into_iter().enumerate() {
        let embedding = match llm_service.generate_embeddings(vec![chunk.text.clone()]).await {
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

        let candidates_str = if candidates.is_empty() {
            "None".to_string()
        } else {
            candidates.iter()
                .map(|c| format!("ID: {}, Title: \"{}\" (Similarity: {:.4})", c.note_id, c.title, c.similarity))
                .collect::<Vec<_>>()
                .join("\n")
        };
        
        let prompt = format!(
            "Analyze this markdown note chunk and recommend how to distribute/organize it.\n\n\
            CHUNK CONTENT:\n\
            ```markdown\n\
            {}\n\
            ```\n\n\
            SUGGESTED TITLE:\n\
            {}\n\n\
            CANDIDATE EXISTING NOTES:\n\
            {}\n\n\
            Choose one of these actions:\n\
            1. \"append\": If the chunk belongs/fits into one of the candidate notes. Provide `target_note_id`.\n\
            2. \"create_child\": If the chunk should be a new sub-note. Provide `new_note_title` and `parent_note_id` (optionally, the ID of a candidate note as its parent, or null if it should be at root level).\n\
            3. \"skip\": If the chunk should be skipped or kept in inbox.\n\n\
            You MUST return a JSON object with the following fields:\n\
            {{\n\
              \"action\": \"append\" | \"create_child\" | \"skip\",\n\
              \"target_note_id\": null or number,\n\
              \"new_note_title\": null or string,\n\
              \"parent_note_id\": null or number,\n\
              \"reason\": \"a brief explanation for this recommendation\"\n\
            }}\n\
            Respond ONLY with the JSON object. Do not include markdown code block syntax (like ```json).",
            chunk.text,
            chunk.suggested_title,
            candidates_str
        );
        
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
    apply_heuristic_linking(&mut plan.chunks);

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

pub fn apply_distribution(
    conn: &mut Connection,
    plan: DraftDistributionPlan,
) -> Result<(), AppError> {
    // Pre-serialize plan before consuming plan.chunks
    let result_json = serde_json::to_string(&plan).unwrap_or_default();

    // 1. First apply all 'append' actions
    for chunk in &plan.chunks {
        if chunk.recommendation.action == "append" {
            if let Some(target_id) = chunk.recommendation.target_note_id {
                let target_note = crate::db::repos::notes::get_by_id(conn, target_id)?
                    .ok_or_else(|| AppError::Other(format!("Target note not found: {}", target_id)))?;
                
                let separator = format!(
                    "\n\n<!-- imported from draft #{} on {} -->\n",
                    plan.draft_id,
                    chrono::Utc::now().to_rfc3339()
                );
                let new_content = if target_note.content.is_empty() {
                    chunk.text.clone()
                } else {
                    format!("{}{}{}", target_note.content, separator, chunk.text)
                };
                
                crate::db::repos::notes::update_content_with_diff(conn, target_id, &new_content)?;
            }
        }
    }

    // 2. Insert new child notes resolving temporary negative parent IDs
    let mut inserted_notes = std::collections::HashMap::new();
    let mut pending_creations: Vec<ChunkDistributionPlan> = plan.chunks.into_iter()
        .filter(|c| c.recommendation.action == "create_child")
        .collect();

    let mut progressed = true;
    while !pending_creations.is_empty() && progressed {
        progressed = false;
        let mut next_pending = Vec::new();

        for chunk in pending_creations {
            let parent_id_opt = chunk.recommendation.parent_note_id;
            
            let can_insert = match parent_id_opt {
                Some(pid) if pid < 0 => {
                    let parent_chunk_idx = (-pid - 1) as usize;
                    inserted_notes.contains_key(&parent_chunk_idx)
                }
                _ => true,
            };

            if can_insert {
                let title = chunk.recommendation.new_note_title.clone()
                    .unwrap_or_else(|| chunk.suggested_title.clone());

                let parent_id = match parent_id_opt {
                    Some(pid) if pid < 0 => {
                        let parent_chunk_idx = (-pid - 1) as usize;
                        inserted_notes.get(&parent_chunk_idx).map(|id| NoteId(*id))
                    }
                    Some(pid) => Some(NoteId(pid)),
                    None => None,
                };

                let new_note = NewNote {
                    title,
                    content: chunk.text,
                    parent_note_id: parent_id,
                    is_draft: false,
                    ..Default::default()
                };
                let new_id = crate::db::repos::notes::insert(conn, &new_note)?;
                inserted_notes.insert(chunk.chunk_index, new_id.0);
                progressed = true;
            } else {
                next_pending.push(chunk);
            }
        }

        pending_creations = next_pending;
    }

    // Safe fallback if circular dependency occurs
    for chunk in pending_creations {
        let title = chunk.recommendation.new_note_title.clone()
            .unwrap_or_else(|| chunk.suggested_title.clone());

        let new_note = NewNote {
            title,
            content: chunk.text,
            parent_note_id: None,
            is_draft: false,
            ..Default::default()
        };
        crate::db::repos::notes::insert(conn, &new_note)?;
    }

    // Move original draft note to archive
    crate::db::repos::notes::archive_note(conn, plan.draft_id)?;

    // Log the applied distribution run using pre-serialized result_json
    let updated = conn.execute(
        "UPDATE distribution_runs 
         SET status = 'applied', result_json = ?, applied_at = strftime('%s','now') 
         WHERE draft_id = ? AND status = 'analyzed'",
        rusqlite::params![result_json, plan.draft_id],
    )?;
    if updated == 0 {
        conn.execute(
            "INSERT INTO distribution_runs (draft_id, plan_json, result_json, generator_version, status, created_at, applied_at)
             VALUES (?, ?, ?, 'v1', 'applied', strftime('%s','now'), strftime('%s','now'))",
            rusqlite::params![plan.draft_id, result_json, result_json],
        )?;
    }

    Ok(())
}
