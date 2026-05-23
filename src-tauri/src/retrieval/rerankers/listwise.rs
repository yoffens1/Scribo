use crate::ai::LlmService;
use crate::retrieval::types::SearchResult;
use std::sync::Arc;
use serde::Deserialize;

#[derive(Deserialize)]
struct RerankResponse {
    order: Vec<usize>,
}

pub async fn rerank_listwise(
    llm: &Arc<LlmService>,
    query: &str,
    candidates: &[SearchResult],
) -> Option<Vec<SearchResult>> {
    let formatted_candidates: Vec<(usize, String)> = candidates.iter()
        .enumerate()
        .map(|(i, r)| (i, r.text.clone().unwrap_or_default()))
        .collect();

    let prompt = crate::ai::prompts::build_rerank_listwise_prompt(query, &formatted_candidates);
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

        if let Ok(parsed) = serde_json::from_str::<RerankResponse>(text_to_parse) {
            let mut reranked = Vec::new();
            for (rank, &orig_idx) in parsed.order.iter().enumerate() {
                if orig_idx < candidates.len() {
                    let mut item = candidates[orig_idx].clone();
                    item.score = 1.0 - (rank as f32 / parsed.order.len() as f32);
                    reranked.push(item);
                }
            }
            // Keep any candidates that weren't returned by rerank but put them at the end
            let returned_set: std::collections::HashSet<usize> = parsed.order.into_iter().collect();
            for (idx, item) in candidates.iter().enumerate() {
                if !returned_set.contains(&idx) {
                    reranked.push(item.clone());
                }
            }
            return Some(reranked);
        }
    }
    None
}
