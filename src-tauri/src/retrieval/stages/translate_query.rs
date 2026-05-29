//! # Translate Query Stage
//!
//! Translates the user's query into the vault's dominant language when they differ.
//! This is important for multilingual vaults where notes are stored in language A
//! but the user issues a query in language B.
//!
//! The stage delegates to [`Translator`](crate::ai::Translator), which wraps the LLM
//! with a strict, JSON-safe translation prompt designed to return only the translated text.

use crate::ai::{LlmService, Translator};
use crate::lang::{detect_language, is_english};
use std::sync::Arc;

/// Decides if translation is necessary based on query language detection,
/// and performs translation using the provided LLM service if needed.
pub async fn maybe_translate_query(
    llm: &Arc<LlmService>,
    query: &str,
    target_lang: &str,
) -> Option<String> {
    // If the target is English and query is already English, skip translation.
    if target_lang == "en" && is_english(query) {
        return None;
    }
    // If we detect the query language is already the target language, skip translation.
    if let Some(detected) = detect_language(query) {
        if detected == target_lang {
            return None;
        }
    }

    let translator = Translator::new(Arc::clone(llm));
    translator.translate(query, target_lang).await.ok()
}
