use crate::ai::LlmService;
use std::collections::HashMap;
use std::sync::Arc;
use serde::Deserialize;

#[derive(Deserialize)]
struct SynonymResponse {
    synonyms: Vec<String>,
}

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

pub async fn expand_llm(
    llm: &Arc<LlmService>,
    query: &str,
    target_lang: &str,
) -> Vec<String> {
    let prompt = crate::ai::prompts::build_synonym_expansion_prompt(query, 3, target_lang);
    if let Ok(resp) = llm.generate_messages(prompt).await {
        let text_to_parse = if let Some(start) = resp.text.find('{') {
            if let Some(end) = resp.text.rfind('}') {
                &resp.text[start..=end]
            } else {
                &resp.text
            }
        } else {
            &resp.text
        };

        if let Ok(parsed) = serde_json::from_str::<SynonymResponse>(text_to_parse) {
            return parsed.synonyms.into_iter()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty() && s.to_lowercase() != query.to_lowercase())
                .collect();
        }
    }
    Vec::new()
}
