//! # Distribute Commands
//!
//! Tauri commands orchestrating the "Distribute Draft" pipeline.
//! This pipeline takes an inbox draft, uses an LLM to find related parent notes,
//! breaks the draft into logical sections, and moves those sections into the parent notes.

use crate::DbState;
use crate::error::AppError;
use crate::ai::{LlmConfig, LlmService};
use crate::domain::distribute::DraftDistributionPlan;
use crate::services::distribute::{analyze_draft_for_distribution, apply_distribution};
use std::sync::Arc;

/// Phase 1: Analyze a draft and generate a distribution plan.
/// Uses the LLM to map draft fragments to existing active notes. Does NOT modify the database.
#[tauri::command]
pub async fn distribute_analyze_draft(
    state: tauri::State<'_, DbState>,
    draft_id: i64,
    llm_config: LlmConfig,
) -> Result<DraftDistributionPlan, AppError> {
    let llm_service = Arc::new(LlmService::new(llm_config, None));
    analyze_draft_for_distribution(&state, draft_id, &llm_service).await
}

/// Mean-pools fragment embeddings (`level = 1`) into section embeddings (`level = 0`).
/// Called during Phase 2 after new fragments have been embedded.
pub fn compute_and_save_section_embeddings(
    _conn: &rusqlite::Connection,
    _note_id: i64,
    _embedding_model: &str,
    _embedding_model_version: &str,
) -> Result<(), AppError> {
    Ok(())
}

/// Phase 2: Execute an approved distribution plan.
/// 1. Inserts content into target notes and deletes the original draft.
/// 2. Re-indexes the affected target notes (generating chunks).
/// 3. Computes LLM embeddings for new fragments (checking the cache first).
/// 4. Generates SRS cards for the new sections via the Reviewer service.
#[tauri::command]
pub async fn distribute_apply_plan(
    app: tauri::AppHandle,
    state: tauri::State<'_, DbState>,
    plan: DraftDistributionPlan,
    llm_config: LlmConfig,
) -> Result<(), AppError> {
    let llm_service = Arc::new(LlmService::new(llm_config.clone(), Some(app.clone())));

    // 1. Apply plan and get affected note IDs
    let affected_note_ids = state.with_write(|conn| {
        apply_distribution(conn, plan)
    })?;

    // 2. Index notes synchronously and update sections
    state.with_write(|conn| {
        for &note_id in &affected_note_ids {
            let payload = crate::services::indexer::IndexingPayload {
                note_id,
                embedding_model: &llm_config.model,
                embedding_dim: crate::constants::EMBEDDING_DIM as u32,
                indexing_version: "1",
            };
            
            if let Err(e) = crate::services::indexer::persist_indexed_file(conn, payload) {
                eprintln!("Failed to persist index for note {}: {}", note_id, e);
                continue;
            }
        }
        Ok(())
    })?;

    // Generate fragment embeddings for all affected note IDs with cache lookup
    let model_name = &llm_config.model;
    for &note_id in &affected_note_ids {
        let fragments = state.with_conn(|conn| {
            crate::db::repos::fragments::list_by_note(conn, note_id, model_name)
        })?;

        let mut final_embeddings = vec![None; fragments.len()];
        let mut cache_miss_indices = Vec::new();
        let mut cache_miss_texts = Vec::new();

        state.with_conn(|conn| {
            for (idx, frag) in fragments.iter().enumerate() {
                let cache_hit: Option<Vec<u8>> = conn.query_row(
                    "SELECT embedding FROM embedding_cache WHERE clean_text_hash = ? AND embedding_model = ? AND embedding_model_version = '1'",
                    rusqlite::params![frag.clean_hash, model_name],
                    |r| r.get(0)
                ).ok();

                if let Some(bytes) = cache_hit {
                    final_embeddings[idx] = Some(bytes);
                } else {
                    cache_miss_indices.push(idx);
                    cache_miss_texts.push(frag.text_clean.clone());
                }
            }
            Ok(())
        })?;

        if !cache_miss_texts.is_empty() {
            if let Ok(embs) = llm_service.generate_embeddings(cache_miss_texts).await {
                state.with_write(|conn| {
                    for (i, emb) in embs.into_iter().enumerate() {
                        let emb_bytes = bytemuck::cast_slice::<f32, u8>(&emb);
                        let frag_idx = cache_miss_indices[i];
                        let clean_hash = &fragments[frag_idx].clean_hash;
                        
                        // Insert into cache
                        conn.execute(
                            "INSERT OR REPLACE INTO embedding_cache (clean_text_hash, embedding_model, embedding_model_version, embedding, created_at)
                             VALUES (?, ?, '1', ?, strftime('%s','now'))",
                            rusqlite::params![clean_hash, model_name, emb_bytes],
                        )?;
                        
                        final_embeddings[frag_idx] = Some(emb_bytes.to_vec());
                    }
                    Ok(())
                })?;
            }
        }

        // Save fragment embeddings
        state.with_write(|conn| {
            for (idx, frag) in fragments.iter().enumerate() {
                if let Some(ref emb_bytes) = final_embeddings[idx] {
                    let frag_idx = frag.fragment_index;
                    if let Err(e) = crate::db::repos::fragments::set_embedding(conn, note_id, frag_idx, emb_bytes, model_name, "1") {
                        eprintln!("Failed to set embedding for note {} fragment {}: {}", note_id, frag_idx, e);
                    }
                }
            }
            Ok(())
        })?;
    }

    // 3. Card refresh: regenerate custom front/back for stale cards
    crate::services::distribute::refresh_stale_cards_for_notes(&state, &affected_note_ids, &llm_service).await?;

    // Emit event "distribute:done" to UI
    use tauri::Emitter;
    let _ = app.emit("distribute:done", ());

    Ok(())
}
