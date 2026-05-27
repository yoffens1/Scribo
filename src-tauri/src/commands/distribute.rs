use crate::DbState;
use crate::error::AppError;
use crate::ai::{LlmConfig, LlmService};
use crate::services::distribute::{DraftDistributionPlan, analyze_draft_for_distribution, apply_distribution};

#[tauri::command]
pub async fn distribute_analyze_draft(
    state: tauri::State<'_, DbState>,
    draft_id: i64,
    llm_config: LlmConfig,
) -> Result<DraftDistributionPlan, AppError> {
    let llm_service = LlmService::new(llm_config, None);
    analyze_draft_for_distribution(&state, draft_id, &llm_service).await
}

#[tauri::command]
pub async fn distribute_apply_plan(
    state: tauri::State<'_, DbState>,
    plan: DraftDistributionPlan,
) -> Result<(), AppError> {
    state.with_write(|conn| {
        apply_distribution(conn, plan)
    })
}
