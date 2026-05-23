use crate::ai::{LlmConfig, LlmService, Message, LlmResponse};
use crate::error::AppError;

#[tauri::command]
pub async fn ai_generate(
    config: LlmConfig,
    messages: Vec<Message>,
) -> Result<LlmResponse, AppError> {
    let service = LlmService::new(config);
    service.generate_messages(messages).await.map_err(|e| AppError::Other(e))
}

#[tauri::command]
pub async fn ai_generate_embeddings(
    config: LlmConfig,
    inputs: Vec<String>,
) -> Result<Vec<Vec<f32>>, AppError> {
    let service = LlmService::new(config);
    service.generate_embeddings(inputs).await.map_err(|e| AppError::Other(e))
}
