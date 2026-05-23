use crate::ai::LlmService;
use std::sync::Arc;

pub async fn run_hyde(
    llm: &Arc<LlmService>,
    query: &str,
    target_lang: &str,
) -> Option<String> {
    let prompt = crate::ai::prompts::build_hyde_prompt(query, target_lang);
    if let Ok(resp) = llm.generate_messages(prompt).await {
        let trimmed = resp.text.trim();
        if trimmed.len() >= 50 {
            return Some(trimmed.to_string());
        }
    }
    None
}
