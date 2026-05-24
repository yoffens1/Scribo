use crate::retrieval::types::SearchResult;
use std::collections::HashMap;

pub fn rrf(
    lists: Vec<(Vec<SearchResult>, f32)>,
    k: f32,
    top_k: usize,
) -> Vec<SearchResult> {
    let mut fused = HashMap::new();
    for (results, weight) in lists {
        for (rank, r) in results.into_iter().enumerate() {
            let id = format!("{}\u{0000}{}", r.fragment_ref.file_path, r.fragment_ref.fragment_index);
            let contribution = weight / (k + rank as f32 + 1.0);
            let entry = fused.entry(id).or_insert((r, 0.0));
            entry.1 += contribution;
        }
    }

    let mut results: Vec<SearchResult> = fused.into_values().map(|(mut r, score)| {
        r.score = score;
        r
    }).collect();

    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(top_k);
    results
}
