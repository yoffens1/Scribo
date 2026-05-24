use async_trait::async_trait;
use reqwest::Client;
use crate::ai::types::{LlmConfig, LlmResponse, Message};
use tauri::AppHandle;

#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Имя провайдера (для логов/ошибок)
    fn name(&self) -> &'static str;

    /// Сгенерировать ответ
    async fn generate(
        &self,
        client: &Client,
        config: &LlmConfig,
        messages: Vec<Message>,
        app: Option<AppHandle>,
    ) -> Result<LlmResponse, String>;
}
