use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use std::time::Instant;
use crate::ai::types::{LlmConfig, LlmResponse, Message};
use super::provider::LlmProvider;
use tauri::AppHandle;

pub struct AnthropicProvider;

#[async_trait]
impl LlmProvider for AnthropicProvider {
    fn name(&self) -> &'static str { "anthropic" }

    async fn generate(
        &self,
        client: &Client,
        config: &LlmConfig,
        messages: Vec<Message>,
        _app: Option<AppHandle>,
    ) -> Result<LlmResponse, String> {
        let start = Instant::now();
        let base = config.base_url.as_deref().unwrap_or("https://api.anthropic.com/v1");
        let url = format!("{}/messages", base);

        let system_message = messages.iter().find(|m| m.role == "system").map(|m| m.content.clone());
        let user_messages: Vec<_> = messages.into_iter().filter(|m| m.role != "system").collect();

        let mut body = json!({
            "model": config.model,
            "messages": user_messages,
            "max_tokens": config.max_tokens.unwrap_or(4096),
        });

        if let Some(sys) = system_message {
            body["system"] = json!(sys);
        }
        if let Some(t) = config.temperature {
            body["temperature"] = json!(t);
        }

        let req = client.post(&url)
            .header("x-api-key", config.api_key.as_deref().unwrap_or(""))
            .header("anthropic-version", "2023-06-01");

        let res = req.json(&body).send().await.map_err(|e| e.to_string())?;

        if !res.status().is_success() {
            return Err(format!("Anthropic API Error: {}", res.text().await.unwrap_or_default()));
        }

        let data: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
        
        let text = data["content"][0]["text"].as_str().unwrap_or_default().to_string();

        let prompt_tokens = data["usage"]["input_tokens"].as_u64().map(|v| v as u32);
        let completion_tokens = data["usage"]["output_tokens"].as_u64().map(|v| v as u32);

        Ok(LlmResponse {
            text,
            model: config.model.clone(),
            latency_ms: start.elapsed().as_millis() as u64,
            prompt_tokens,
            completion_tokens,
            cached_tokens: None,
        })
    }
}
