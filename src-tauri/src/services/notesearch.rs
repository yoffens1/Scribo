use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub file_path: String,
    pub score: i64,
}

pub struct FuzzySearch {
    matcher: SkimMatcherV2,
    notes: Vec<String>,
}

impl FuzzySearch {
    pub fn new(notes: Vec<String>) -> Self {
        Self {
            matcher: SkimMatcherV2::default(),
            notes,
        }
    }

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

        // Sort descending by score
        results.sort_by(|a, b| b.score.cmp(&a.score));
        results.truncate(limit);
        results
    }
}
