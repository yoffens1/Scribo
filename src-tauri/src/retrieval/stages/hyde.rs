//! # HyDE — Hypothetical Document Embeddings
//!
//! Generates a *hypothetical* answer document for the user's query using the LLM,
//! then embeds that document instead of (or in addition to) the raw query.
//!
//! ## Why it works
//!
//! Query texts are often short and may not share vocabulary with the stored fragments.
//! A synthetic document written by the LLM in the same style and language as the vault
//! tends to be much closer in embedding space to the actual answer fragments,
//! improving recall for semantically dense corpora.
//!
//! ## Limitations
//!
//! - The hypothetical document is `vector_only` — it is **never** used for FTS5/keyword search,
//!   because synthetic text doesn't match real BM25 token frequencies.
//! - Responses shorter than 50 characters are discarded as low-quality.

use crate::ai::LlmService;
use std::sync::Arc;

/// Runs the HyDE stage: asks the LLM to write a short hypothetical answer for `query`
/// in `target_lang`, then returns the trimmed text if it meets the minimum length threshold.
///
/// Returns `None` when the LLM call fails or the response is too short to be useful.
pub async fn run_hyde(
    llm: &Arc<LlmService>,
    query: &str,
    target_lang: &str,
) -> Option<String> {
    let prompt = crate::ai::prompts::build_hyde_prompt(query, target_lang);
    if let Ok(resp) = llm.generate_messages(prompt).await {
        let trimmed = resp.text.trim();
        // Discard very short responses — they are usually refusals or empty completions.
        if trimmed.len() >= 50 {
            return Some(trimmed.to_string());
        }
    }
    None
}
