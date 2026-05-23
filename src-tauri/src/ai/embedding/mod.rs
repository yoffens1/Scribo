pub mod ollama;
pub mod openai;

use crate::ai::types::EmbedderConfig;
use reqwest::Client;
use futures::stream::{self, StreamExt};

pub struct Embedder {
    client: Client,
    config: EmbedderConfig,
    dim: Option<usize>,
}

impl Embedder {
    pub fn new(config: EmbedderConfig) -> Self {
        let dim = Self::get_known_dim(config.model.as_deref().unwrap_or(""));
        Self {
            client: Client::new(),
            config,
            dim,
        }
    }

    fn get_known_dim(model: &str) -> Option<usize> {
        match model {
            "qwen3-embedding:latest" => Some(4096),
            "nomic-embed-text" => Some(768),
            "mxbai-embed-large" => Some(1024),
            "all-minilm" => Some(384),
            "text-embedding-3-small" => Some(1536),
            "text-embedding-3-large" => Some(3072),
            "text-embedding-ada-002" => Some(1536),
            _ => None,
        }
    }

    pub fn get_dimensions(&self) -> Option<usize> {
        self.dim
    }

    pub async fn embed(&self, text: &str) -> Result<Vec<f32>, String> {
        match self.config.provider.as_str() {
            "openai" => openai::embed(
                &self.client,
                self.config.base_url.as_deref(),
                self.config.model.as_deref(),
                self.config.api_key.as_deref(),
                text
            ).await,
            "ollama" => ollama::embed(
                &self.client,
                self.config.base_url.as_deref(),
                self.config.model.as_deref(),
                text
            ).await,
            other => Err(format!("Unsupported embedder provider: {}", other)),
        }
    }

    pub async fn embed_query(&self, text: &str) -> Result<Vec<f32>, String> {
        let model = self.config.model.as_deref().unwrap_or("");
        let prompt = if model.starts_with("qwen3") {
            format!("Instruct: Given a query, retrieve relevant passages\nQuery: {}", text)
        } else {
            text.to_string()
        };
        self.embed(&prompt).await
    }

    pub async fn embed_batch(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, String> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        
        match self.config.provider.as_str() {
            "openai" => openai::embed_batch(
                &self.client,
                self.config.base_url.as_deref(),
                self.config.model.as_deref(),
                self.config.api_key.as_deref(),
                texts
            ).await,
            "ollama" => {
                let concurrency_limit = 5;
                let results = stream::iter(texts.into_iter())
                    .map(|text| async move {
                        self.embed(&text).await
                    })
                    .buffer_unordered(concurrency_limit)
                    .collect::<Vec<_>>()
                    .await;
                
                let mut final_vecs = Vec::with_capacity(results.len());
                for res in results {
                    final_vecs.push(res?);
                }
                Ok(final_vecs)
            },
            other => Err(format!("Unsupported embedder provider: {}", other)),
        }
    }
}
