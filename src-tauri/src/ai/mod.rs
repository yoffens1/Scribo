//! # AI Module
//!
//! Top-level façade for all AI capabilities in Scribo.
//!
//! ## Subsystems
//!
//! | Module | Responsibility |
//! |---|---|
//! | [`types`]      | Shared data types: `LlmConfig`, `Message`, `LlmResponse`, `EmbedderConfig`, `Provider` |
//! | [`llm`]        | `LlmService` — unified chat/completion API across OpenAI, Anthropic, Gemini, and local llama.cpp |
//! | [`embedding`]  | `Embedder` — text embedding with L2 normalisation; model-specific prompt formatting |
//! | [`models`]     | Local GGUF model lifecycle: directory resolution, GGUF header scanning, LRU model cache |
//! | [`prompts`]    | Typed prompt builders for each LLM task (HyDE, rerank, synonyms, translate, distribute, refinery) |
//! | [`parsing`]    | LLM response post-processing: extract JSON from markdown code blocks or mixed text |
//! | [`translator`] | Thin async wrapper that drives `prompts::translation` through `LlmService` |
//!
//! ## Key re-exports
//!
//! - [`cosine_similarity`] / [`cosine_similarity_normalized`] — used by the retrieval pipeline and vector search.
//! - [`extract_json_payload`] — strips markdown fences from LLM JSON responses.

pub mod types;
pub mod llm;
pub mod prompts;
pub mod embedding;
pub mod models;
pub mod translator;
pub mod parsing;

pub use types::{LlmConfig, LlmResponse, Message, EmbedderConfig};
pub use llm::LlmService;
pub use embedding::Embedder;
pub use embedding::similarity::{cosine_similarity, cosine_similarity_normalized};
pub use translator::Translator;
pub use parsing::{extract_json_payload, extract_json_object, extract_json_array};
pub use prompts::{
    build_atomize_prompt, build_taxonomy_prompt, FragmentForTaxonomy,
    build_placement_prompt, build_hyde_prompt, build_synonym_expansion_prompt,
    build_rerank_listwise_prompt, build_rerank_scoring_prompt,
    build_translate_prompt, build_translate_strict_prompt
};
