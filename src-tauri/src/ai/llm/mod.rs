pub mod anthropic;
pub mod gemini;
pub mod ollama;
pub mod openai;

use reqwest::Client;
use crate::ai::types::{LlmConfig, LlmResponse, Message};

pub struct LlmService {
    client: Client,
    config: LlmConfig,
}

impl LlmService {
    pub fn new(config: LlmConfig) -> Self {
        Self {
            client: Client::new(),
            config,
        }
    }

    pub async fn generate_messages(&self, mut messages: Vec<Message>) -> Result<LlmResponse, String> {
        if let Some(sys_prompt) = &self.config.system_prompt {
            if !messages.iter().any(|m| m.role == "system") {
                messages.insert(0, Message {
                    role: "system".to_string(),
                    content: sys_prompt.clone(),
                });
            }
        }

        match self.config.backend.as_str() {
            "openai" | "openrouter" | "deepseek" => openai::generate(
                &self.client,
                self.config.base_url.as_deref(),
                &self.config.model,
                self.config.api_key.as_deref(),
                self.config.temperature,
                self.config.max_tokens,
                self.config.response_format.as_deref(),
                messages,
            ).await,
            "ollama" => ollama::generate(
                &self.client,
                self.config.base_url.as_deref(),
                &self.config.model,
                self.config.temperature,
                self.config.max_tokens,
                self.config.response_format.as_deref(),
                messages,
            ).await,
            "anthropic" => anthropic::generate(
                &self.client,
                self.config.base_url.as_deref(),
                &self.config.model,
                self.config.api_key.as_deref(),
                self.config.temperature,
                self.config.max_tokens,
                messages,
            ).await,
            "gemini" => gemini::generate(
                &self.client,
                self.config.base_url.as_deref(),
                &self.config.model,
                self.config.api_key.as_deref(),
                self.config.temperature,
                self.config.max_tokens,
                self.config.response_format.as_deref(),
                messages,
            ).await,
            other => Err(format!("Unsupported backend: {}", other)),
        }
    }

    pub async fn generate_embeddings(&self, inputs: Vec<String>) -> Result<Vec<Vec<f32>>, String> {
        use crate::ai::types::EmbedderConfig;
        use crate::ai::Embedder;
        
        let config = EmbedderConfig {
            provider: self.config.backend.clone(),
            model: Some(self.config.model.clone()),
            api_key: self.config.api_key.clone(),
            base_url: self.config.base_url.clone(),
        };
        
        let embedder = Embedder::new(config);
        embedder.embed_batch(inputs).await
    }
}
