use async_trait::async_trait;
use reqwest::Client;
use crate::ai::types::EmbedderConfig;
use super::provider::EmbeddingProvider;

pub struct LocalEmbedder;

#[async_trait]
impl EmbeddingProvider for LocalEmbedder {
    fn name(&self) -> &'static str { "local" }

    async fn embed(&self, _client: &Client, config: &EmbedderConfig, text: &str) -> Result<Vec<f32>, String> {
        let model_id = config.model.as_deref().unwrap_or("nomic-embed-text");
        let manager = crate::ai::models::manager::get_model_manager();
        let loaded_model = manager.load_model(model_id)?;
        let engine = loaded_model.engine.clone();
        let t = text.to_string();
        tokio::task::spawn_blocking(move || {
            engine.embed_sync(&t)
        }).await.map_err(|e| e.to_string())?
    }
}
