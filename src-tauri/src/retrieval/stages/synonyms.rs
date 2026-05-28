//! # Synonym Expansion Stage
//!
//! Generates additional query variants by expanding the original query text
//! with domain-specific synonyms. Two strategies are supported:
//!
//! - **Static** ([`expand_static`]) — fast O(1) dictionary lookup against
//!   `data/synonyms.json`. Suitable for well-known abbreviations (e.g. `"ml"` → `"machine learning"`).
//! - **LLM** ([`expand_llm`])    — asks the model to produce up to 3 semantically close
//!   alternative phrasings. Higher latency, but generalises to any domain.
//!
//! Expanded terms become independent [`QueryVariant`](crate::retrieval::types::QueryVariant)s
//! with a reduced weight (see `RetrievalTuning::synonym_weight`), so they broaden recall
//! without dominating the relevance ranking.

use crate::ai::LlmService;
use std::collections::HashMap;
use std::sync::Arc;
use serde::Deserialize;

/// Expected JSON envelope returned by the LLM synonym-expansion prompt.
#[derive(Deserialize)]
struct SynonymResponse {
    synonyms: Vec<String>,
}

/// Looks up `query` in the provided static dictionary and returns all mapped synonyms.
/// The lookup is case-insensitive (query is lowercased before matching).
/// Returns an empty `Vec` when no entry is found.
pub fn expand_static(
    query: &str,
    dict: &HashMap<String, Vec<String>>,
) -> Vec<String> {
    let lower = query.to_lowercase();
    if let Some(syns) = dict.get(&lower) {
        syns.clone()
    } else {
        Vec::new()
    }
}

/// Asks the LLM to generate up to 3 synonymous query phrasings in `target_lang`.
///
/// The response is parsed as `{ "synonyms": ["...", "..."] }`.
/// Any synonym identical to the original query (case-insensitive) or empty is discarded.
/// Returns an empty `Vec` on LLM failure or parse error.
pub async fn expand_llm(
    llm: &Arc<LlmService>,
    query: &str,
    target_lang: &str,
) -> Vec<String> {
    let prompt = crate::ai::prompts::build_synonym_expansion_prompt(query, 3, target_lang);
    if let Ok(resp) = llm.generate_messages(prompt).await {
        let text_to_parse = crate::ai::extract_json_object(&resp.text);

        if let Ok(parsed) = serde_json::from_str::<SynonymResponse>(text_to_parse) {
            return parsed.synonyms.into_iter()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty() && s.to_lowercase() != query.to_lowercase())
                .collect();
        }
    }
    Vec::new()
}
