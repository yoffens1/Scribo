use async_trait::async_trait;
use reqwest::Client;
use crate::ai::types::EmbedderConfig;

#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    fn name(&self) -> &'static str;

    async fn embed(
        &self,
        client: &Client,
        config: &EmbedderConfig,
        text: &str,
    ) -> Result<Vec<f32>, String>;

    /// Дефолтная реализация — параллелит embed.
    async fn embed_batch(
        &self,
        client: &Client,
        config: &EmbedderConfig,
        texts: Vec<String>,
    ) -> Result<Vec<Vec<f32>>, String> {
        use futures::stream::{self, StreamExt};
        let results = stream::iter(texts.into_iter())
            .map(|t| async move { self.embed(client, config, &t).await })
            .buffer_unordered(5)
            .collect::<Vec<_>>().await;
        
        let mut vecs = Vec::with_capacity(results.len());
        for res in results {
            vecs.push(res?);
        }
        Ok(vecs)
    }
}
