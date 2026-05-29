//! # Pure Retrieval Parameter Calibration Logic
//!
//! Contains mathematical calculations for optimizing retrieval parameters (RRF k, embedding weight, min score)
//! based on Mean Reciprocal Rank (MRR) metrics over a set of evaluation samples.

use crate::retrieval::types::SearchResult;
use crate::retrieval::search::rrf;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvalSample {
    pub expected_title: String,
    pub weight: f32,
    pub keyword_hits: Vec<SearchResult>,
    pub vector_hits: Vec<SearchResult>,
}

/// Evaluates Mean Reciprocal Rank (MRR) for given samples and parameters.
pub fn mean_reciprocal_rank(samples: &[EvalSample], emb_w: f32, rrf_k: f32) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let mut total_rr = 0.0;
    for item in samples {
        let fused = rrf(
            vec![
                (item.keyword_hits.clone(), 1.0),
                (item.vector_hits.clone(), emb_w),
            ],
            rrf_k,
            crate::constants::FUSION_CANDIDATES,
        );

        // Find rank of expected note title (case-insensitive)
        let mut found_rank = None;
        for (rank, r) in fused.iter().enumerate() {
            if let Some(ref title) = r.note_title {
                if title.to_lowercase() == item.expected_title.to_lowercase() {
                    found_rank = Some(rank);
                    break;
                }
            }
        }

        if let Some(rank) = found_rank {
            total_rr += item.weight * (1.0 / (rank as f32 + 1.0));
        }
    }
    total_rr / samples.len() as f32
}

/// Grid search parameters.
pub struct GridSearchParameters {
    pub embedding_weights: Vec<f32>,
    pub rrf_ks: Vec<f32>,
}

impl Default for GridSearchParameters {
    fn default() -> Self {
        Self {
            embedding_weights: crate::constants::GRID_EMBEDDING_WEIGHTS.to_vec(),
            rrf_ks: crate::constants::GRID_RRF_KS.to_vec(),
        }
    }
}

/// Runs a grid search to optimize RRF parameters.
/// Returns: `(best_embedding_weight, best_rrf_k, best_mrr)`
pub fn grid_search(
    samples: &[EvalSample],
    params: &GridSearchParameters,
) -> (f32, f32, f32) {
    let mut best_mrr = -1.0;
    let mut best_w = crate::constants::DEFAULT_EMBEDDING_WEIGHT;
    let mut best_k = crate::constants::DEFAULT_RRF_K;

    for &w in &params.embedding_weights {
        for &k in &params.rrf_ks {
            let mrr = mean_reciprocal_rank(samples, w, k);
            if mrr > best_mrr {
                best_mrr = mrr;
                best_w = w;
                best_k = k;
            }
        }
    }

    (best_w, best_k, best_mrr)
}

/// Calibrates the min_score threshold using a safety margin based on target scores.
pub fn calibrate_min_score(samples: &[EvalSample], best_w: f32, best_k: f32) -> f32 {
    let mut target_scores = Vec::new();
    for item in samples {
        let fused = rrf(
            vec![
                (item.keyword_hits.clone(), 1.0),
                (item.vector_hits.clone(), best_w),
            ],
            best_k,
            crate::constants::FUSION_CANDIDATES,
        );
        for (rank, r) in fused.iter().enumerate() {
            if let Some(ref title) = r.note_title {
                if title.to_lowercase() == item.expected_title.to_lowercase() && rank <= 5 {
                    target_scores.push(r.score);
                    break;
                }
            }
        }
    }

    if !target_scores.is_empty() {
        let min_val = target_scores.iter().copied().fold(f32::INFINITY, f32::min);
        // Apply safety margin to ensure we catch target documents
        (min_val * crate::constants::MIN_SCORE_SAFETY_MARGIN)
            .min(crate::constants::MIN_SCORE_CEILING)
            .max(crate::constants::MIN_SCORE_FLOOR)
    } else {
        crate::constants::MIN_SCORE_FALLBACK
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
    fn test_retrieval_calibration() {
        let sample1 = EvalSample {
            expected_title: "Doc A".to_string(),
            weight: 1.0,
            keyword_hits: vec![
                make_result(1, 0, "Doc A"),
                make_result(2, 0, "Doc B"),
            ],
            vector_hits: vec![
                make_result(2, 0, "Doc B"),
                make_result(1, 0, "Doc A"),
            ],
        };

        let samples = vec![sample1];

        // Evaluate MRR with a set of parameters
        let mrr_equal = mean_reciprocal_rank(&samples, 1.0, 60.0);
        assert!(mrr_equal > 0.0);

        // Run grid search
        let params = GridSearchParameters {
            embedding_weights: vec![0.0, 1.0, 2.0],
            rrf_ks: vec![60.0],
        };
        let (best_w, best_k, best_mrr) = grid_search(&samples, &params);
        assert_eq!(best_k, 60.0);
        assert!(best_mrr >= mrr_equal);

        // Calibrate min score
        let min_score = calibrate_min_score(&samples, best_w, best_k);
        assert!(min_score > 0.0);
    }
}
