use reqwest::Client;
use serde_json::json;

pub async fn embed(
    client: &Client,
    base_url: Option<&str>,
    model: Option<&str>,
    text: &str,
) -> Result<Vec<f32>, String> {
    let url = format!("{}/api/embeddings", base_url.unwrap_or("http://localhost:11434"));
    let m = model.unwrap_or("qwen3-embedding:latest");

    let body = json!({
        "model": m,
        "prompt": text,
    });

    let res = client.post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !res.status().is_success() {
        return Err(format!("API Error: {}", res.text().await.unwrap_or_default()));
    }

    let data: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
    
    if let Some(embedding_array) = data["embedding"].as_array() {
        let vec: Vec<f32> = embedding_array.iter()
            .filter_map(|v| v.as_f64().map(|f| f as f32))
            .collect();
        Ok(vec)
    } else {
        Err("Invalid response shape from Ollama".into())
    }
}
