use crate::refinery::RefineryPipeline;
use crate::refinery::types::PlacementPlan;
use crate::ai::{LlmConfig, LlmService};
use crate::error::AppError;
use std::sync::Arc;

#[tauri::command]
pub async fn refinery_run_pipeline(
    state: tauri::State<'_, crate::DbState>,
    source_path: String,
    content: String,
    llm_config: LlmConfig,
) -> Result<PlacementPlan, AppError> {
    let llm_service = Arc::new(LlmService::new(llm_config, None));
    let pipeline = RefineryPipeline::new(llm_service);
    
    pipeline.refine(&source_path, &content, &state).await.map_err(|e| AppError::Other(e))
}
