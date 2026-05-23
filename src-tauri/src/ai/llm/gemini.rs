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
    let base = base_url.unwrap_or("https://generativelanguage.googleapis.com/v1beta");
    
    let key = api_key.unwrap_or("");
    let url = format!("{}/models/{}:generateContent?key={}", base, model, key);

    let system_message = messages.iter().find(|m| m.role == "system").map(|m| m.content.clone());
    let user_messages: Vec<_> = messages.into_iter().filter(|m| m.role != "system").map(|m| {
        json!({
            "role": if m.role == "assistant" { "model" } else { "user" },
            "parts": [{ "text": m.content }]
        })
    }).collect();

    let mut body = json!({
        "contents": user_messages,
    });

    if let Some(sys) = system_message {
        body["systemInstruction"] = json!({
            "parts": [{ "text": sys }]
        });
    }

    let mut generation_config = json!({});
    if let Some(t) = temperature {
        generation_config["temperature"] = json!(t);
    }
    if let Some(m) = max_tokens {
        generation_config["maxOutputTokens"] = json!(m);
    }
    if response_format == Some("json") {
        generation_config["responseMimeType"] = json!("application/json");
    }

    if generation_config.as_object().map(|o| !o.is_empty()).unwrap_or(false) {
        body["generationConfig"] = generation_config;
    }

    let res = client.post(&url).json(&body).send().await.map_err(|e| e.to_string())?;

    if !res.status().is_success() {
        return Err(format!("API Error: {}", res.text().await.unwrap_or_default()));
    }

    let data: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
    
    let text = data["candidates"][0]["content"]["parts"][0]["text"].as_str().unwrap_or_default().to_string();

    let prompt_tokens = data["usageMetadata"]["promptTokenCount"].as_u64().map(|v| v as u32);
    let completion_tokens = data["usageMetadata"]["candidatesTokenCount"].as_u64().map(|v| v as u32);

    Ok(LlmResponse {
        text,
        model: model.to_string(),
        latency_ms: start.elapsed().as_millis() as u64,
        prompt_tokens,
        completion_tokens,
        cached_tokens: None,
    })
}
