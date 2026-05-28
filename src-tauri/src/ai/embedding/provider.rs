//! # Embedding Provider Trait
//!
//! Defines the [`EmbeddingProvider`] trait that all embedding backends must implement.
//! The default `embed_batch` implementation fans out to concurrent individual `embed` calls
//! with a concurrency limit of 5, which is overridden by providers (like OpenAI) that
//! support native batch requests.

use async_trait::async_trait;
use reqwest::Client;
use crate::ai::types::EmbedderConfig;

/// Backend-agnostic embedding interface.
///
/// Implementors: [`OpenAiEmbedder`](crate::ai::embedding::openai::OpenAiEmbedder),
/// [`LocalEmbedder`](crate::ai::embedding::local::LocalEmbedder).
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Human-readable backend name used in error messages and logs.
    fn name(&self) -> &'static str;

    /// Embeds a single text string. Returns an **unnormalised** `f32` vector.
    /// L2 normalisation is applied by the [`Embedder`](crate::ai::embedding::Embedder) wrapper.
    async fn embed(
        &self,
        client: &Client,
        config: &EmbedderConfig,
        text: &str,
    ) -> Result<Vec<f32>, String>;

    /// Embeds a batch of texts. The default implementation fans out `embed` calls
    /// concurrently with up to 5 in-flight requests. Providers with a native batch
    /// endpoint should override this for efficiency.
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
