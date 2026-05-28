//! # Note Search Service
//!
//! Fuzzy note-title search backed by the `skim` fuzzy-matcher algorithm.
//!
//! This is intentionally **not** a full-text search — it operates on the list of note
//! file paths / titles provided by the caller (typically the frontend's in-memory list).
//! It is used for quick note-picker interactions, not knowledge retrieval.
//!
//! For semantic retrieval over fragment content, see [`crate::retrieval`].

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use serde::{Deserialize, Serialize};

/// A single fuzzy search result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    /// The file path or title that matched the query.
    pub file_path: String,
    /// Skim relevance score — higher is a better match.
    /// Score 0 means barely matching; there is no fixed upper bound.
    pub score: i64,
}

/// A reusable fuzzy-search engine over a fixed set of note paths/titles.
///
/// Construct once per search session (or per request if the list is small) and
/// call [`search`](FuzzySearch::search) for each query.
pub struct FuzzySearch {
    matcher: SkimMatcherV2,
    /// The candidate strings to match against.
    notes: Vec<String>,
}

impl FuzzySearch {
    /// Creates a new fuzzy-search instance over the provided note paths.
    pub fn new(notes: Vec<String>) -> Self {
        Self {
            matcher: SkimMatcherV2::default(),
            notes,
        }
    }

    /// Returns up to `limit` notes that fuzzy-match `query`, sorted by descending score.
    /// Notes with no match are excluded entirely.
    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchHit> {
        let mut results = Vec::new();

        for file_path in &self.notes {
            if let Some(score) = self.matcher.fuzzy_match(file_path, query) {
                results.push(SearchHit {
                    file_path: file_path.clone(),
                    score,
                });
            }
        }

        results.sort_by(|a, b| b.score.cmp(&a.score));
        results.truncate(limit);
        results
    }
}
