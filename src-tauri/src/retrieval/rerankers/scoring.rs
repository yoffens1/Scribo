//! # Scoring Reranker
//!
//! The LLM is given the query and a numbered list of candidate snippets and is asked
//! to assign each one a relevance score on a 0–`max_score` scale.
//!
//! ## Flow
//!
//! 1. Format candidates as `[(id, text), ...]` and pass them to the scoring prompt.
//! 2. Parse the LLM response as `[{ "id": usize, "score": f32 }, ...]`.
//! 3. Normalise scores to [0, 1] by dividing by `max_score`.
//! 4. Sort candidates in-place descending by the new score.
//!
//! Missing candidates (not returned by the LLM) keep their original fusion score.
//! Parse failures and LLM errors are logged as warnings without panicking.

use crate::ai::LlmService;
use crate::retrieval::types::SearchResult;
use std::sync::Arc;
use std::collections::HashMap;
use serde::Deserialize;

/// One item in the JSON array returned by the scoring prompt.
#[derive(Deserialize)]
struct ScoringItem {
    /// 0-based index matching the candidate list sent to the LLM.
    id: usize,
    /// Raw score on the 0–`max_score` scale.
    score: f32,
}

/// Reranks `candidates` in-place by asking the LLM to score each one.
///
/// # Arguments
/// - `llm`       — shared LLM service.
/// - `query`     — original user query (context for the LLM).
/// - `candidates`— mutable slice of pre-fetched results; sorted in-place.
/// - `max_score` — expected maximum raw score from the LLM (normalisation denominator).
pub async fn rerank_scoring(
    llm: &Arc<LlmService>,
    query: &str,
    candidates: &mut [SearchResult],
    max_score: f32,
) {
    let formatted_candidates: Vec<(usize, String)> = candidates.iter()
        .enumerate()
        .map(|(i, r)| (i, r.text.clone().unwrap_or_default()))
        .collect();

    let prompt = crate::ai::prompts::build_rerank_scoring_prompt(query, &formatted_candidates);
    match llm.generate_messages(prompt).await {
        Ok(resp) => {
            let text_to_parse = crate::ai::extract_json_array(&resp.text);

            match serde_json::from_str::<Vec<ScoringItem>>(text_to_parse) {
                Ok(parsed) => {
                    // Build an id→score map for O(1) lookup.
                    let mut score_map = HashMap::new();
                    for item in parsed {
                        score_map.insert(item.id, item.score);
                    }
                    // Apply normalised scores; unscored candidates keep their old score.
                    for (i, c) in candidates.iter_mut().enumerate() {
                        if let Some(&score) = score_map.get(&i) {
                            c.score = score / max_score;
                        }
                    }
                    candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
                }
                Err(e) => {
                    tracing::warn!(error = %e, response = %resp.text, "Failed to parse scoring reranking response");
                }
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, "Scoring reranking LLM call failed");
        }
    }
}
