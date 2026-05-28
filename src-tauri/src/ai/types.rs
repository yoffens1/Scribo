//! # AI Types
//!
//! Shared data structures used across the LLM, embedding, and prompt subsystems.
//! All types derive `Serialize`/`Deserialize` with `camelCase` field names to match
//! the frontend Tauri command interface.

use serde::{Deserialize, Serialize};

/// A single turn in a chat conversation.
///
/// `role` must be one of `"system"`, `"user"`, or `"assistant"`.
/// Different backends map these to their own role conventions internally.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub role: String,
    pub content: String,
}

/// Configuration for the active LLM backend.
///
/// `backend` is a provider slug — see [`Provider::from_str`] for accepted values.
/// `response_format` controls whether the model is instructed to output `"json"` or `"text"`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LlmConfig {
    /// Provider slug: `"openai"`, `"anthropic"`, `"gemini"`, `"local"`, etc.
    pub backend: String,
    /// Model identifier string (e.g. `"gpt-4o"`, `"claude-3-5-sonnet-20241022"`).
    pub model: String,
    pub api_key: Option<String>,
    /// Custom base URL for OpenAI-compatible endpoints (Ollama, LM Studio, vLLM, OpenRouter).
    pub base_url: Option<String>,
    /// Optional system prompt injected automatically by `LlmService::generate_messages`
    /// if no system message is already present in the conversation.
    pub system_prompt: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    /// `"json"` instructs supported backends to enable structured output mode.
    pub response_format: Option<String>,
}

/// Token usage and timing metadata returned with every LLM response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmResponse {
    pub text: String,
    pub model: String,
    /// Wall-clock latency of the HTTP request in milliseconds.
    pub latency_ms: u64,
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    /// Tokens served from the provider's KV cache (OpenAI prompt caching).
    pub cached_tokens: Option<u32>,
}

/// Configuration for the embedding model, decoupled from `LlmConfig`
/// so a different provider/model can be used for embeddings vs. text generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbedderConfig {
    /// Provider slug: `"openai"`, `"local"`, etc.
    pub provider: String,
    pub model: Option<String>,
    pub api_key: Option<String>,
    /// Custom base URL for OpenAI-compatible embedding endpoints.
    pub base_url: Option<String>,
}

/// Internal enum resolved from a provider slug string.
/// `OpenAi` covers all OpenAI-compatible APIs (Ollama, LM Studio, vLLM, OpenRouter, DeepSeek).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    OpenAi,
    Anthropic,
    Gemini,
    /// Local llama.cpp via `llama-cpp-2`.
    Local,
}

impl Provider {
    /// Resolves a provider slug to the corresponding enum variant.
    /// Returns `Err` for unknown slugs.
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "openai" | "openrouter" | "deepseek" | "ollama" | "lmstudio" | "vllm"
                                  => Ok(Self::OpenAi),
            "anthropic" | "claude" => Ok(Self::Anthropic),
            "gemini" | "google"    => Ok(Self::Gemini),
            "local" | "llamacpp"   => Ok(Self::Local),
            other                  => Err(format!("Unknown provider: {}", other)),
        }
    }
}
