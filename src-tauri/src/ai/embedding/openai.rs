use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use crate::ai::types::EmbedderConfig;
use super::provider::EmbeddingProvider;

pub struct OpenAiEmbedder;

#[async_trait]
impl EmbeddingProvider for OpenAiEmbedder {
    fn name(&self) -> &'static str { "openai" }

    async fn embed(&self, client: &Client, cfg: &EmbedderConfig, text: &str) -> Result<Vec<f32>, String> {
        let base = cfg.base_url.as_deref().unwrap_or("https://api.openai.com/v1");
        let url = format!("{}/embeddings", base);
        let m = cfg.model.as_deref().unwrap_or("text-embedding-3-small");
        let key = cfg.api_key.as_deref().unwrap_or("");

        let body = json!({
            "model": m,
            "input": text,
        });

        let res = client.post(&url)
            .bearer_auth(key)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !res.status().is_success() {
            return Err(format!("OpenAI API Error: {}", res.text().await.unwrap_or_default()));
        }

        let data: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
        
        if let Some(embedding_array) = data["data"][0]["embedding"].as_array() {
            let vec: Vec<f32> = embedding_array.iter()
                .filter_map(|v| v.as_f64().map(|f| f as f32))
                .collect();
            Ok(vec)
        } else {
            Err("Invalid response shape from OpenAI".into())
        }
    }

    async fn embed_batch(&self, client: &Client, cfg: &EmbedderConfig, texts: Vec<String>) -> Result<Vec<Vec<f32>>, String> {
        let base = cfg.base_url.as_deref().unwrap_or("https://api.openai.com/v1");
        let url = format!("{}/embeddings", base);
        let m = cfg.model.as_deref().unwrap_or("text-embedding-3-small");
        let key = cfg.api_key.as_deref().unwrap_or("");

        let body = json!({
            "model": m,
            "input": texts,
        });

        let res = client.post(&url)
            .bearer_auth(key)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !res.status().is_success() {
            return Err(format!("OpenAI API Error: {}", res.text().await.unwrap_or_default()));
        }

        let data: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
        
        if let Some(data_array) = data["data"].as_array() {
            let mut vecs = Vec::with_capacity(data_array.len());
            for item in data_array {
                if let Some(embedding_array) = item["embedding"].as_array() {
                    let vec: Vec<f32> = embedding_array.iter()
                        .filter_map(|v| v.as_f64().map(|f| f as f32))
                        .collect();
                    vecs.push(vec);
                } else {
                    return Err("Invalid embedding array in batch response".into());
                }
            }
            Ok(vecs)
        } else {
            Err("Invalid batch response shape from OpenAI".into())
        }
    }
}
