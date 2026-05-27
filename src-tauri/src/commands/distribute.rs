use crate::DbState;
use crate::error::AppError;
use crate::ai::{LlmConfig, LlmService};
use crate::domain::distribute::DraftDistributionPlan;
use crate::services::distribute::{analyze_draft_for_distribution, apply_distribution};
use std::sync::Arc;

#[tauri::command]
pub async fn distribute_analyze_draft(
    state: tauri::State<'_, DbState>,
    draft_id: i64,
    llm_config: LlmConfig,
) -> Result<DraftDistributionPlan, AppError> {
    let llm_service = Arc::new(LlmService::new(llm_config, None));
    analyze_draft_for_distribution(&state, draft_id, &llm_service).await
}

#[tauri::command]
pub async fn distribute_apply_plan(
    app: tauri::AppHandle,
    state: tauri::State<'_, DbState>,
    plan: DraftDistributionPlan,
    llm_config: LlmConfig,
) -> Result<(), AppError> {
    let llm_service = Arc::new(LlmService::new(llm_config, Some(app.clone())));

    // 1. Apply plan and get affected note IDs
    let affected_note_ids = state.with_write(|conn| {
        apply_distribution(conn, plan)
    })?;

    // 2. Index notes synchronously and update sections
    state.with_write(|conn| {
        for &note_id in &affected_note_ids {
            let payload = crate::services::indexer::IndexingPayload {
                note_id,
                embedding_model: "granite-embedding-97M-multilingual-r2-BF16",
                embedding_dim: 384,
                indexing_version: "1",
            };
            
            if let Err(e) = crate::services::indexer::persist_indexed_file(conn, payload) {
                eprintln!("Failed to persist index for note {}: {}", note_id, e);
                continue;
            }
        }
        Ok(())
    })?;

    // Generate fragment embeddings for all affected note IDs
    for &note_id in &affected_note_ids {
        let fragments = state.with_conn(|conn| {
            crate::db::repos::fragments::list_by_note(conn, note_id)
        })?;

        let texts: Vec<String> = fragments.iter().map(|f| f.text_clean.clone()).collect();
        if !texts.is_empty() {
            if let Ok(embs) = llm_service.generate_embeddings(texts).await {
                state.with_write(|conn| {
                    for (i, emb) in embs.into_iter().enumerate() {
                        let emb_bytes = bytemuck::cast_slice::<f32, u8>(&emb);
                        let frag_idx = fragments[i].fragment_index;
                        if let Err(e) = crate::db::repos::fragments::set_embedding(conn, note_id, frag_idx, emb_bytes) {
                            eprintln!("Failed to set embedding for note {} fragment {}: {}", note_id, frag_idx, e);
                        }
                    }
                    Ok(())
                })?;
            }
        }
    }

    // 3. Card refresh: regenerate custom front/back for stale cards
    crate::services::distribute::refresh_stale_cards_for_notes(&state, &affected_note_ids, &llm_service).await?;

    // Emit event "distribute:done" to UI
    use tauri::Emitter;
    let _ = app.emit("distribute:done", ());

    Ok(())
}
