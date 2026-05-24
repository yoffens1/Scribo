pub mod provider;
pub mod openai;
pub mod local;

use reqwest::Client;
use provider::EmbeddingProvider;
use crate::ai::types::{EmbedderConfig, Provider};

pub struct Embedder {
    client: Client,
    config: EmbedderConfig,
    dim: Option<usize>,
}

impl Embedder {
    pub fn new(config: EmbedderConfig) -> Self {
        let dim = known_dim(config.model.as_deref().unwrap_or(""));
        Self { client: Client::new(), config, dim }
    }

    pub fn get_dimensions(&self) -> Option<usize> { self.dim }

    fn impl_(&self) -> Result<Box<dyn EmbeddingProvider>, String> {
        Ok(match Provider::from_str(&self.config.provider)? {
            Provider::OpenAi => Box::new(openai::OpenAiEmbedder),
            Provider::Local  => Box::new(local::LocalEmbedder),
            other => return Err(format!("Embedding provider {:?} not supported", other)),
        })
    }

    pub async fn embed(&self, text: &str) -> Result<Vec<f32>, String> {
        let model = self.config.model.as_deref().unwrap_or("");
        let formatted_text = format_passage(text, model);
        let mut vec = self.impl_()?.embed(&self.client, &self.config, &formatted_text).await?;
        l2_normalize(&mut vec);
        Ok(vec)
    }

    pub async fn embed_query(&self, text: &str) -> Result<Vec<f32>, String> {
        let model = self.config.model.as_deref().unwrap_or("");
        let formatted_text = format_query(text, model);
        let mut vec = self.impl_()?.embed(&self.client, &self.config, &formatted_text).await?;
        l2_normalize(&mut vec);
        Ok(vec)
    }

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

fn format_passage(text: &str, model: &str) -> String {
    let m = model.to_lowercase();
    if m.contains("e5") {
        format!("passage: {}", text)
    } else {
        text.to_string()
    }
}

fn l2_normalize(v: &mut [f32]) {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

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
        _ if m.contains("qwen3-embedding")        => Some(4096),
        _ if m.contains("text-embedding-3-small") => Some(1536),
        _ if m.contains("text-embedding-3-large") => Some(3072),
        _ if m.contains("text-embedding-ada-002") => Some(1536),
        _ => None,
    }
}
