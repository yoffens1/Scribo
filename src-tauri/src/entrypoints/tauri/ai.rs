//! # AI Commands
//!
//! Tauri commands bridging the frontend to the `ai` module.
//! Handles LLM text generation, embedding generation, and local GGUF model management.

use crate::ai::{LlmConfig, Message, LlmResponse};
use crate::error::AppError;
use crate::ai::models::scanner::{LocalModel, scan_models};
use crate::DbState;
use tauri::State;

/// Sends a conversation to the configured LLM and returns the response.
/// Uses `DbState::get_llm_service` to reuse the connection pool across requests.
#[tauri::command]
pub async fn ai_generate(
    app: tauri::AppHandle,
    state: State<'_, DbState>,
    config: LlmConfig,
    messages: Vec<Message>,
) -> Result<LlmResponse, AppError> {
    let service = state.get_llm_service(&config, Some(app));
    service.generate_messages(messages).await.map_err(|e| AppError::Other(e))
}

/// Generates embeddings for a batch of input strings using the configured provider.
#[tauri::command]
pub async fn ai_generate_embeddings(
    state: State<'_, DbState>,
    config: LlmConfig,
    inputs: Vec<String>,
) -> Result<Vec<Vec<f32>>, AppError> {
    let service = state.get_llm_service(&config, None);
    service.generate_embeddings(inputs).await.map_err(|e| AppError::Other(e))
}

/// Scans the local `~/.local/share/scribo/models` directory for `.gguf` files
/// and parses their headers to return metadata (architecture, context length, etc.).
#[tauri::command]
pub async fn ai_list_local_models() -> Result<Vec<LocalModel>, AppError> {
    Ok(scan_models())
}

/// Ejects a local model from RAM. Called manually by the user or automatically
/// when switching models to free up system memory.
#[tauri::command]
pub async fn ai_local_unload_model(id: String) -> Result<(), AppError> {
    let manager = crate::ai::models::manager::get_model_manager();
    manager.unload_model(&id);
    Ok(())
}
