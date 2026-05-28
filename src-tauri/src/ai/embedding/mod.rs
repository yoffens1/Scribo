//! # Embedding Module
//!
//! Provides the [`Embedder`] — a unified interface for converting text into dense vector
//! representations, regardless of the underlying provider.
//!
//! ## Pipeline
//!
//! ```text
//! raw text
//!    │
//!    ▼  format_query / format_passage   (model-specific instruction prefixes)
//! formatted text
//!    │
//!    ▼  EmbeddingProvider::embed / embed_batch   (HTTP or local inference)
//! raw f32 vector
//!    │
//!    ▼  l2_normalize
//! unit-norm f32 vector   →  stored in DB / used with cosine_similarity_normalized
//! ```
//!
//! ## Instruction Prefixes
//!
//! Some embedding models require task-specific prefixes for optimal retrieval quality:
//!
//! | Model family | Query prefix | Passage prefix |
//! |---|---|---|
//! | `bge-*` | `"Represent this sentence for searching relevant passages: ..."` | none |
//! | `e5-*`  | `"query: ..."` | `"passage: ..."` |
//! | `qwen3-embedding` | `"Instruct: ... \nQuery: ..."` | none |
//! | Others  | none | none |
//!
//! ## L2 Normalisation
//!
//! All output vectors are L2-normalised before being returned.
//! This enables [`cosine_similarity_normalized`](crate::ai::cosine_similarity_normalized)
//! (dot product only, ~2× faster) to be used safely in the vector search hot path.

pub mod provider;
pub mod openai;
pub mod local;
pub mod similarity;

use reqwest::Client;
use provider::EmbeddingProvider;
use crate::ai::types::{EmbedderConfig, Provider};

/// Unified embedding client.
/// Wraps a provider-specific [`EmbeddingProvider`] and applies prompt formatting
/// and L2 normalisation transparently.
pub struct Embedder {
    client: Client,
    config: EmbedderConfig,
    /// Known output dimensionality, resolved from the model name at construction time.
    /// `None` for unknown models.
    dim: Option<usize>,
}

impl Embedder {
    /// Creates an `Embedder` for `config`. Resolves the expected vector dimension
    /// from the model name via [`known_dim`] (no network call).
    pub fn new(config: EmbedderConfig) -> Self {
        let dim = known_dim(config.model.as_deref().unwrap_or(""));
        Self { client: Client::new(), config, dim }
    }

    /// Returns the known embedding dimensionality, or `None` for unrecognised models.
    pub fn get_dimensions(&self) -> Option<usize> { self.dim }

    /// Resolves the concrete [`EmbeddingProvider`] from the config's provider slug.
    fn impl_(&self) -> Result<Box<dyn EmbeddingProvider>, String> {
        Ok(match Provider::from_str(&self.config.provider)? {
            Provider::OpenAi => Box::new(openai::OpenAiEmbedder),
            Provider::Local  => Box::new(local::LocalEmbedder),
            other => return Err(format!("Embedding provider {:?} not supported", other)),
        })
    }

    /// Embeds a **passage** (document to be indexed).
    /// Applies model-specific passage prefix formatting before sending to the provider.
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>, String> {
        let model = self.config.model.as_deref().unwrap_or("");
        let formatted_text = format_passage(text, model);
        let mut vec = self.impl_()?.embed(&self.client, &self.config, &formatted_text).await?;
        l2_normalize(&mut vec);
        Ok(vec)
    }

    /// Embeds a **query** (user's search input).
    /// Applies model-specific query prefix formatting before sending to the provider.
    pub async fn embed_query(&self, text: &str) -> Result<Vec<f32>, String> {
        let model = self.config.model.as_deref().unwrap_or("");
        let formatted_text = format_query(text, model);
        let mut vec = self.impl_()?.embed(&self.client, &self.config, &formatted_text).await?;
        l2_normalize(&mut vec);
        Ok(vec)
    }

    /// Batch-embeds multiple **passages** in a single API call (where supported).
    /// Falls back to concurrent individual requests for providers without native batch support.
    pub async fn embed_batch(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, String> {
        if texts.is_empty() { return Ok(Vec::new()); }
        let model = self.config.model.as_deref().unwrap_or("");
        let formatted_texts: Vec<String> = texts.into_iter().map(|t| format_passage(&t, model)).collect();
        let mut vecs = self.impl_()?.embed_batch(&self.client, &self.config, formatted_texts).await?;
        for vec in &mut vecs {
            l2_normalize(vec);
        }
        Ok(vecs)
    }
}

/// Applies the model-specific query instruction prefix.
/// Returns the text unchanged for models that don't require a prefix.
fn format_query(text: &str, model: &str) -> String {
    let m = model.to_lowercase();
    if m.contains("bge") {
        format!("Represent this sentence for searching relevant passages: {}", text)
    } else if m.contains("e5") {
        format!("query: {}", text)
    } else if m.contains("qwen3") {
        format!("Instruct: Given a query, retrieve relevant passages\nQuery: {}", text)
    } else {
        text.to_string()
    }
}

/// Applies the model-specific passage instruction prefix.
/// Currently only `e5-*` models require a prefix for passages.
fn format_passage(text: &str, model: &str) -> String {
    let m = model.to_lowercase();
    if m.contains("e5") {
        format!("passage: {}", text)
    } else {
        text.to_string()
    }
}

/// In-place L2 normalisation. Divides every component by the vector's Euclidean norm.
/// No-ops on zero vectors (avoids division by zero).
fn l2_normalize(v: &mut [f32]) {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

/// Returns the known output dimensionality for well-known embedding models.
/// Uses substring matching on the lowercase model name — returns `None` for unknowns.
fn known_dim(model: &str) -> Option<usize> {
    let m = model.to_lowercase();
    match () {
        _ if m.contains("bge-small")              => Some(384),
        _ if m.contains("bge-base")               => Some(768),
        _ if m.contains("bge-large")              => Some(1024),
        _ if m.contains("e5-small")               => Some(384),
        _ if m.contains("e5-base")                => Some(768),
        _ if m.contains("e5-large")               => Some(1024),
        _ if m.contains("minilm-l6")              => Some(384),
        _ if m.contains("nomic-embed")            => Some(768),
        _ if m.contains("arctic-embed-s")         => Some(384),
        _ if m.contains("granite-embedding")      => Some(384),
        _ if m.contains("qwen3-embedding")        => Some(4096),
        _ if m.contains("text-embedding-3-small") => Some(1536),
        _ if m.contains("text-embedding-3-large") => Some(3072),
        _ if m.contains("text-embedding-ada-002") => Some(1536),
        _                                         => None,
    }
}
