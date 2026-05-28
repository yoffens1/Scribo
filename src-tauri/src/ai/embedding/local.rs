//! # Local Embedding Provider
//!
//! Runs embedding inference locally via llama.cpp using the [`ModelManager`](crate::ai::models::manager::ModelManager).
//!
//! ## Execution model
//!
//! llama.cpp inference is CPU/GPU-bound and not async-safe, so it is dispatched to a
//! `tokio::task::spawn_blocking` thread to avoid blocking the async executor.
//! The model is loaded on first use and kept in the LRU model cache.

use async_trait::async_trait;
use reqwest::Client;
use crate::ai::types::EmbedderConfig;
use super::provider::EmbeddingProvider;

/// Local llama.cpp embedding backend.
/// Resolves the model from the model cache by `model_id` (without the `.gguf` extension).
pub struct LocalEmbedder;

#[async_trait]
impl EmbeddingProvider for LocalEmbedder {
    fn name(&self) -> &'static str { "local" }

    async fn embed(&self, _client: &Client, config: &EmbedderConfig, text: &str) -> Result<Vec<f32>, String> {
        let model_id = config.model.as_deref().unwrap_or("nomic-embed-text");
        let manager = crate::ai::models::manager::get_model_manager();
        // Load (or retrieve from cache) the GGUF model.
        let loaded_model = manager.load_model(model_id)?;
        let engine = loaded_model.engine.clone();
        let t = text.to_string();
        // Dispatch to blocking thread — llama.cpp inference is synchronous.
        tokio::task::spawn_blocking(move || {
            engine.embed_sync(&t)
        }).await.map_err(|e| e.to_string())?
    }
}
