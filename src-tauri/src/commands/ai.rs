use crate::ai::{LlmConfig, LlmService, Message, LlmResponse};
use crate::error::AppError;
use crate::ai::models::scanner::{LocalModel, scan_models};

#[tauri::command]
pub async fn ai_generate(
    app: tauri::AppHandle,
    config: LlmConfig,
    messages: Vec<Message>,
) -> Result<LlmResponse, AppError> {
    let service = LlmService::new(config, Some(app));
    service.generate_messages(messages).await.map_err(|e| AppError::Other(e))
}

#[tauri::command]
pub async fn ai_generate_embeddings(
    config: LlmConfig,
    inputs: Vec<String>,
) -> Result<Vec<Vec<f32>>, AppError> {
    let service = LlmService::new(config, None);
    service.generate_embeddings(inputs).await.map_err(|e| AppError::Other(e))
}

#[tauri::command]
pub async fn ai_list_local_models() -> Result<Vec<LocalModel>, AppError> {
    Ok(scan_models())
}

#[tauri::command]
pub async fn ai_local_unload_model(id: String) -> Result<(), AppError> {
    let manager = crate::ai::models::manager::get_model_manager();
    manager.unload_model(&id);
    Ok(())
}
