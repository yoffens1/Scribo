pub mod provider;
pub mod openai;
pub mod anthropic;
pub mod gemini;
pub mod local;

use reqwest::Client;
use crate::ai::types::{LlmConfig, LlmResponse, Message, Provider};
use provider::LlmProvider;
use tauri::AppHandle;

pub struct LlmService {
    client: Client,
    config: LlmConfig,
    app: Option<AppHandle>,
}

impl LlmService {
    pub fn new(config: LlmConfig, app: Option<AppHandle>) -> Self {
        Self { client: Client::new(), config, app }
    }

    pub fn config(&self) -> &LlmConfig {
        &self.config
    }

    fn provider_for(p: Provider) -> Box<dyn LlmProvider> {
        match p {
            Provider::OpenAi    => Box::new(openai::OpenAiProvider),
            Provider::Anthropic => Box::new(anthropic::AnthropicProvider),
            Provider::Gemini    => Box::new(gemini::GeminiProvider),
            Provider::Local     => Box::new(local::LocalProvider),
        }
    }

    pub async fn generate_messages(&self, mut messages: Vec<Message>) -> Result<LlmResponse, String> {
        // System prompt inject
        if let Some(sys) = &self.config.system_prompt {
            if !messages.iter().any(|m| m.role == "system") {
                messages.insert(0, Message {
                    role: "system".into(),
                    content: sys.clone(),
                });
            }
        }

        let provider = Provider::from_str(&self.config.backend)?;
        let impl_ = Self::provider_for(provider);
        impl_.generate(&self.client, &self.config, messages, self.app.clone()).await
    }

    pub async fn generate_embeddings(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, String> {
        let embedder_cfg = crate::ai::types::EmbedderConfig {
            provider: self.config.backend.clone(),
            model: Some(self.config.model.clone()),
            api_key: self.config.api_key.clone(),
            base_url: self.config.base_url.clone(),
        };
        let embedder = crate::ai::embedding::Embedder::new(embedder_cfg);
        embedder.embed_batch(texts).await
    }
}
