use reqwest::Client;
use serde_json::json;
use std::time::Instant;
use crate::ai::types::{LlmResponse, Message};

pub async fn generate(
    client: &Client,
    base_url: Option<&str>,
    model: &str,
    api_key: Option<&str>,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
    response_format: Option<&str>,
    messages: Vec<Message>,
) -> Result<LlmResponse, String> {
    let start = Instant::now();
    let url = format!("{}/chat/completions", base_url.unwrap_or("https://api.openai.com/v1"));

    let mut body = json!({
        "model": model,
        "messages": messages,
    });

    if let Some(t) = temperature {
        body["temperature"] = json!(t);
    }
    if let Some(m) = max_tokens {
        body["max_tokens"] = json!(m);
    }
    if response_format == Some("json") {
        body["response_format"] = json!({ "type": "json_object" });
    }

    let mut req = client.post(&url);
    if let Some(key) = api_key {
        req = req.bearer_auth(key);
    }

    let res = req.json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !res.status().is_success() {
        return Err(format!("API Error: {}", res.text().await.unwrap_or_default()));
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
        model: model.to_string(),
        latency_ms: start.elapsed().as_millis() as u64,
        prompt_tokens,
        completion_tokens,
        cached_tokens: None,
    })
}
