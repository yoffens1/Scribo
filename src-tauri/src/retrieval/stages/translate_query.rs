//! # Translate Query Stage
//!
//! Translates the user's query into the vault's dominant language when they differ.
//! This is important for multilingual vaults where notes are stored in language A
//! but the user issues a query in language B.
//!
//! The stage delegates to [`Translator`](crate::ai::Translator), which wraps the LLM
//! with a strict, JSON-safe translation prompt designed to return only the translated text.

use crate::ai::{LlmService, Translator};
use std::sync::Arc;

/// Translates `query` into `target_lang` using the provided LLM service.
///
/// Returns `Some(translated_text)` on success, or `None` when the LLM call
/// fails or the translator returns an error.
pub async fn run_translation(
    llm: &Arc<LlmService>,
    query: &str,
    target_lang: &str,
) -> Option<String> {
    let translator = Translator::new(Arc::clone(llm));
    translator.translate(query, target_lang).await.ok()
}
