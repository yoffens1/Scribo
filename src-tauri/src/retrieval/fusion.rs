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

use crate::retrieval::types::{SearchResult, FragmentRef};
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
    // Accumulate fused scores keyed by FragmentRef (Hash + Eq).
    let mut fused: HashMap<FragmentRef, (SearchResult, f32)> = HashMap::new();

    for (results, weight) in lists {
        for (rank, r) in results.into_iter().enumerate() {
            let key = r.fragment_ref.clone();
            let contribution = weight / (k + rank as f32 + 1.0);
            // Insert or accumulate: keep the first `SearchResult` as the carrier,
            // and sum up contributions from all lists.
            let entry = fused.entry(key).or_insert((r, 0.0));
            entry.1 += contribution;
        }
    }

    // Write the accumulated scores back into the result structs.
    let mut results: Vec<SearchResult> = fused.into_values().map(|(mut r, score)| {
        r.score = score;
        r
    }).collect();

    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(top_k);
    results
}
