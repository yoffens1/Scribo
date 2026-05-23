use reqwest::Client;
use serde_json::json;
use std::time::Instant;
use crate::ai::types::{LlmResponse, Message};

pub async fn generate(
    client: &Client,
    base_url: Option<&str>,
    model: &str,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
    response_format: Option<&str>,
    messages: Vec<Message>,
) -> Result<LlmResponse, String> {
    let start = Instant::now();
    let url = format!("{}/api/chat", base_url.unwrap_or("http://localhost:11434"));

    let mut options = json!({});
    if let Some(t) = temperature {
        options["temperature"] = json!(t);
    }
    if let Some(m) = max_tokens {
        options["num_predict"] = json!(m);
    }

    let mut body = json!({
        "model": model,
        "messages": messages,
        "stream": false,
        "options": options,
    });

    if response_format == Some("json") {
        body["format"] = json!("json");
    }

    let res = client.post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !res.status().is_success() {
        return Err(format!("API Error: {}", res.text().await.unwrap_or_default()));
    }

    let data: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
    
    let text = data["message"]["content"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    Ok(LlmResponse {
        text,
        model: model.to_string(),
        latency_ms: start.elapsed().as_millis() as u64,
        prompt_tokens: None,
        completion_tokens: None,
        cached_tokens: None,
    })
}
