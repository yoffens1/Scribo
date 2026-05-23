use crate::ai::LlmService;
use crate::retrieval::types::SearchResult;
use std::sync::Arc;
use std::collections::HashMap;
use serde::Deserialize;

#[derive(Deserialize)]
struct ScoringItem {
    id: usize,
    score: f32,
}

pub async fn rerank_scoring(
    llm: &Arc<LlmService>,
    query: &str,
    candidates: &mut [SearchResult],
) {
    let formatted_candidates: Vec<(usize, String)> = candidates.iter()
        .enumerate()
        .map(|(i, r)| (i, r.text.clone().unwrap_or_default()))
        .collect();

    let prompt = crate::ai::prompts::build_rerank_scoring_prompt(query, &formatted_candidates);
    if let Ok(resp) = llm.generate_messages(prompt).await {
        let text_to_parse = if let Some(start) = resp.text.find('[') {
            if let Some(end) = resp.text.rfind(']') {
                &resp.text[start..=end]
            } else {
                &resp.text
            }
        } else {
            &resp.text
        };

        if let Ok(parsed) = serde_json::from_str::<Vec<ScoringItem>>(text_to_parse) {
            let mut score_map = HashMap::new();
            for item in parsed {
                score_map.insert(item.id, item.score);
            }
            for (i, c) in candidates.iter_mut().enumerate() {
                if let Some(&score) = score_map.get(&i) {
                    c.score = score / 10.0;
                }
            }
            candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        }
    }
}
