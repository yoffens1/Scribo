use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use std::time::Instant;
use crate::ai::types::{LlmConfig, LlmResponse, Message};
use super::provider::LlmProvider;
use tauri::AppHandle;

pub struct OpenAiProvider;

#[async_trait]
impl LlmProvider for OpenAiProvider {
    fn name(&self) -> &'static str { "openai" }

    async fn generate(
        &self,
        client: &Client,
        config: &LlmConfig,
        messages: Vec<Message>,
        _app: Option<AppHandle>,
    ) -> Result<LlmResponse, String> {
        let start = Instant::now();
        let base = config.base_url.as_deref().unwrap_or("https://api.openai.com/v1");
        let url = format!("{}/chat/completions", base);

        let mut body = json!({
            "model": config.model,
            "messages": messages,
        });

        if let Some(t) = config.temperature { body["temperature"] = json!(t); }
        if let Some(m) = config.max_tokens { body["max_tokens"] = json!(m); }
        if config.response_format.as_deref() == Some("json") {
            body["response_format"] = json!({ "type": "json_object" });
        }

        let mut req = client.post(&url);
        if let Some(key) = &config.api_key {
            req = req.bearer_auth(key);
        }

        let res = req.json(&body).send().await.map_err(|e| e.to_string())?;

        if !res.status().is_success() {
            return Err(format!("OpenAI API Error: {}", res.text().await.unwrap_or_default()));
        }

        let data: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
        
        let text = data["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or_default()
            .to_string();

        let prompt_tokens = data["usage"]["prompt_tokens"].as_u64().map(|v| v as u32);
        let completion_tokens = data["usage"]["completion_tokens"].as_u64().map(|v| v as u32);

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
