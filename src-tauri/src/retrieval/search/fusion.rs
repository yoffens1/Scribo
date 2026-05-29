//! # Reciprocal Rank Fusion (RRF)
//!
//! Merges multiple ranked result lists into a single ranked list.
//!
//! ## Algorithm
//!
//! For each document `d` appearing in any input list, its fused score is:
//!
//! ```text
//! score(d) = Σ  weight_i / (k + rank_i(d) + 1)
//!             i
//! ```
//!
//! where:
//! - `rank_i(d)` is the 0-based position of `d` in list `i` (lower = better).
//! - `weight_i` is the importance weight of list `i`.
//! - `k` is a smoothing constant that dampens the influence of top-ranked documents
//!   (the original RRF paper uses k=60).
//!
//! Documents are deduplicated by [`FragmentRef`], which implements `Hash + Eq`.
//! The final list is sorted descending by score and truncated to `top_k`.

use crate::retrieval::types::{SearchResult, FragmentRef, ScoreDebug};
use std::collections::HashMap;

/// Fuses multiple ranked `SearchResult` lists using Reciprocal Rank Fusion.
///
/// # Arguments
/// - `lists`  — `(results, weight)` pairs. Each list is independently ranked.
/// - `k`      — RRF smoothing constant. Typical value: 60.0.
/// - `top_k`  — Number of results to return after fusion.
pub fn rrf(
    lists: Vec<(Vec<SearchResult>, f32)>,
    k: f32,
    top_k: usize,
) -> Vec<SearchResult> {
    // Accumulate: (SearchResult, accumulated_rrf_score, keyword_rank, vector_rank)
    let mut fused: HashMap<FragmentRef, (SearchResult, f32, Option<usize>, Option<usize>)> = HashMap::new();

    for (list_idx, (results, weight)) in lists.into_iter().enumerate() {
        for (rank, r) in results.into_iter().enumerate() {
            let key = r.fragment_ref.clone();
            let contribution = weight / (k + rank as f32 + 1.0);
            
            let entry = fused.entry(key).or_insert_with(|| {
                (r.clone(), 0.0, None, None)
            });
            entry.1 += contribution;
            if list_idx == 0 {
                entry.2 = Some(rank);
            } else if list_idx == 1 {
                entry.3 = Some(rank);
            }
        }
    }

    // Write the accumulated scores back into the result structs.
    let mut results: Vec<SearchResult> = fused.into_values().map(|(mut r, score, kw_rank, vec_rank)| {
        r.score = score;
        r.debug = Some(ScoreDebug {
            bm25_rank: kw_rank,
            vector_rank: vec_rank,
            rrf_score: score,
            term_boost: 0.0,
            rerank_score: None,
        });
        r
    }).collect();

    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(top_k);
    results
}

/// Applies a score boost to search results for exact keyword query term matches.
pub fn apply_term_boost(results: &mut Vec<SearchResult>, query: &str) {
    let query_lower = query.to_lowercase();
    let query_terms: Vec<&str> = query_lower.split_whitespace().collect();
    if !query_terms.is_empty() {
        for r in results.iter_mut() {
            if let Some(ref text) = r.text {
                let text_lower = text.to_lowercase();
                let mut matches_count = 0;
                for term in &query_terms {
                    if term.chars().count() > 1 {
                        matches_count += text_lower.matches(term).count();
                    }
                }
                if matches_count > 0 {
                    let boost = (matches_count as f32 * 0.05).min(0.5);
                    r.score += boost;
                    if let Some(ref mut dbg) = r.debug {
                        dbg.term_boost = boost;
                    } else {
                        r.debug = Some(ScoreDebug {
                            bm25_rank: None,
                            vector_rank: None,
                            rrf_score: 0.0,
                            term_boost: boost,
                            rerank_score: None,
                        });
                    }
                }
            }
        }
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval::types::{SearchResult, FragmentRef};

    fn make_result(note_id: i64, fragment_index: i64, text: &str) -> SearchResult {
        SearchResult {
            fragment_ref: FragmentRef {
                note_id: crate::domain::note::NoteId(note_id),
                fragment_index,
            },
            score: 0.0,
            text: Some(text.to_string()),
            note_title: Some(text.to_string()),
            debug: None,
        }
    }

    #[test]
    fn test_rrf_scoring_and_weights() {
        // Prepare list 1 (keyword): R0=A, R1=B
        let list1 = vec![
            make_result(1, 0, "Doc A"),
            make_result(2, 0, "Doc B"),
        ];

        // Prepare list 2 (vector): R0=B, R1=C
        let list2 = vec![
            make_result(2, 0, "Doc B"),
            make_result(3, 0, "Doc C"),
        ];

        // RRF with k = 60.0
        // Weights: List 1 = 1.0, List 2 = 0.5
        let lists = vec![
            (list1, 1.0),
            (list2, 0.5),
        ];

        let fused = rrf(lists, 60.0, 10);

        // Doc A score: 1.0 / (60 + 0 + 1) = 1.0 / 61.0 ≈ 0.016393
        // Doc B score: 1.0 / (60 + 1 + 1) + 0.5 / (60 + 0 + 1) = 1.0 / 62.0 + 0.5 / 61.0 ≈ 0.024326
        // Doc C score: 0.5 / (60 + 1 + 1) = 0.5 / 62.0 ≈ 0.008064

        assert_eq!(fused.len(), 3);
        
        // B should be ranked first
        assert_eq!(fused[0].fragment_ref.note_id.0, 2);
        assert!((fused[0].score - (1.0 / 62.0 + 0.5 / 61.0)).abs() < 1e-5);

        // A should be ranked second
        assert_eq!(fused[1].fragment_ref.note_id.0, 1);
        assert!((fused[1].score - (1.0 / 61.0)).abs() < 1e-5);

        // C should be ranked third
        assert_eq!(fused[2].fragment_ref.note_id.0, 3);
        assert!((fused[2].score - (0.5 / 62.0)).abs() < 1e-5);

        // Verify score debug is populated
        let debug_b = fused[0].debug.as_ref().unwrap();
        assert_eq!(debug_b.bm25_rank, Some(1)); // 0-indexed rank 1 in list 1
        assert_eq!(debug_b.vector_rank, Some(0)); // 0-indexed rank 0 in list 2
    }

    #[test]
    fn test_apply_term_boost() {
        let mut results = vec![
            make_result(1, 0, "This is an atom fragment speaking"),
            make_result(2, 0, "No match here"),
        ];

        apply_term_boost(&mut results, "atom molecule");

        // "atom" is present in result 1, so it should get a boost of 0.05
        assert!(results[0].score > 0.0);
        let debug = results[0].debug.as_ref().unwrap();
        assert_eq!(debug.term_boost, 0.05);

        // Result 2 has no match, so score remains 0.0 and debug is None or boost is 0.0
        assert_eq!(results[1].score, 0.0);
        assert!(results[1].debug.is_none() || results[1].debug.as_ref().unwrap().term_boost == 0.0);
    }
}

