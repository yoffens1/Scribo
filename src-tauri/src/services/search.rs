//! # Search Service
//!
//! Provides two main search mechanisms:
//! 1. In-memory fuzzy note-title matching backed by the `skim` fuzzy-matcher algorithm.
//! 2. Semantic hybrid retrieval (FTS5 + vector + RRF) orchestrating the lower-level retrieval engine.

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use serde::{Deserialize, Serialize};

use crate::retrieval::{retrieve, RetrievalConfig, SearchResult as RetSearchResult, RetrieveOptions};
use crate::error::AppError;
use crate::db::DbState;

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

/// Orchestrates semantic retrieval for a query using the lower-level retrieval engine.
pub async fn run_retrieval_query(
    query: String,
    query_embedding: Option<Vec<f32>>,
    config: RetrievalConfig,
    options: Option<RetrieveOptions>,
    state: &DbState,
) -> Result<Vec<RetSearchResult>, AppError> {
    let opts = options.unwrap_or(RetrieveOptions {
        top_k: None,
        filters: None,
        target_level: None,
    });
    retrieve(state, &query, query_embedding.as_deref(), &config, &opts).await
}
