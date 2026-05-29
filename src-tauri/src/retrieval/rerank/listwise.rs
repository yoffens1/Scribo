//! # Listwise Reranker
//!
//! The LLM receives the query and a numbered list of candidate snippets and returns
//! a **permutation** of their indices sorted by relevance — most relevant first.
//!
//! ## Flow
//!
//! 1. Format candidates as `[(id, text), ...]` and call the listwise prompt.
//! 2. Parse `{ "order": [2, 0, 4, 1, 3, ...] }` from the response.
//! 3. Rebuild the list in the returned order, assigning synthetic scores
//!    that decay linearly from 1.0 (rank 0) to ~0.0 (rank n-1).
//! 4. Candidates not mentioned by the LLM are appended at the end with their original scores.
//!
//! ## Trade-offs vs Scoring
//!
//! Listwise reranking tends to produce better orderings because the LLM can compare
//! candidates relative to each other rather than scoring them independently.
//! However, it requires the LLM to return a complete permutation, making it sensitive
//! to truncation or hallucinated indices.

use crate::ai::LlmService;
use crate::retrieval::types::SearchResult;
use std::sync::Arc;
use serde::Deserialize;

/// Expected JSON structure returned by the listwise prompt.
#[derive(Deserialize)]
struct RerankResponse {
    /// Permutation of 0-based candidate indices, most relevant first.
    order: Vec<usize>,
}

/// Reranks `candidates` by asking the LLM to return a relevance permutation.
///
/// Returns `Some(reranked_list)` on success, or `None` when the LLM call fails
/// or the response cannot be parsed. The caller should fall back to the original
/// fusion order on `None`.
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
    match llm.generate_messages(prompt).await {
        Ok(resp) => {
            let text_to_parse = crate::ai::extract_json_object(&resp.text);

            match serde_json::from_str::<RerankResponse>(text_to_parse) {
                Ok(parsed) => {
                    let mut reranked = Vec::new();

                    // Reconstruct in the LLM-specified order, assigning decaying synthetic scores.
                    for (rank, &orig_idx) in parsed.order.iter().enumerate() {
                        if orig_idx < candidates.len() {
                            let mut item = candidates[orig_idx].clone();
                            // Score decays linearly: rank 0 → 1.0, rank n-1 → ~0.0.
                            let new_score = 1.0 - (rank as f32 / parsed.order.len() as f32);
                            item.score = new_score;
                            if let Some(ref mut dbg) = item.debug {
                                dbg.rerank_score = Some(new_score);
                            } else {
                                item.debug = Some(crate::retrieval::types::ScoreDebug {
                                    bm25_rank: None,
                                    vector_rank: None,
                                    rrf_score: 0.0,
                                    term_boost: 0.0,
                                    rerank_score: Some(new_score),
                                });
                            }
                            reranked.push(item);
                        }
                    }

                    // Append any candidates the LLM didn't mention (safety net for truncation).
                    let returned_set: std::collections::HashSet<usize> = parsed.order.into_iter().collect();
                    for (idx, item) in candidates.iter().enumerate() {
                        if !returned_set.contains(&idx) {
                            reranked.push(item.clone());
                        }
                    }

                    Some(reranked)
                }
                Err(e) => {
                    tracing::warn!(error = %e, response = %resp.text, "Failed to parse listwise reranking response");
                    None
                }
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, "Listwise reranking LLM call failed");
            None
        }
    }
}
