//! # Global System Constants and Defaults
//!
//! Centralized repository for all magic numbers, default values, and thresholds
//! used across the hybrid retrieval pipeline, indexer, and AI services.

// ── Embedding model configuration ──
pub const EMBEDDING_MODEL: &str = "multilingual-e5-large-instruct-F16";
pub const EMBEDDING_DIM: usize = 1024;
pub const EMBEDDING_CTX: usize = 2048;        // n_ctx context window
pub const INDEXING_VERSION: &str = "1";

// ── RRF / fusion parameters ──
pub const DEFAULT_RRF_K: f32 = 60.0;
pub const DEFAULT_EMBEDDING_WEIGHT: f32 = 1.5;
pub const DEFAULT_TERM_BOOST_WEIGHT: f32 = 0.05;
pub const FUSION_CANDIDATES: usize = 50;       // Must align with search/calibration limit

// ── Pipeline heuristics ──
pub const HYDE_WORD_THRESHOLD: usize = 4;
pub const VAULT_LANG_SAMPLE_SIZE: usize = 50;
pub const MIN_TEXT_LEN_FOR_LANG: usize = 10;

// ── min_score calibration ranges ──
pub const MIN_SCORE_SAFETY_MARGIN: f32 = 0.9;
pub const MIN_SCORE_CEILING: f32 = 0.05;
pub const MIN_SCORE_FLOOR: f32 = 0.001;
pub const MIN_SCORE_FALLBACK: f32 = 0.005;

// ── LLM defaults ──
pub const DEFAULT_LLM_MAX_TOKENS: u32 = 2048;
pub const FALLBACK_LLM_MODEL: &str = "google/gemini-2.5-flash";
pub const OPENROUTER_BASE_URL: &str = "https://openrouter.ai/api/v1";

// ── Grid search space ──
pub const GRID_EMBEDDING_WEIGHTS: &[f32] = &[0.0, 0.2, 0.5, 0.8, 1.0, 1.2, 1.5, 1.8, 2.0, 2.5, 3.0];
pub const GRID_RRF_KS: &[f32] = &[10.0, 20.0, 40.0, 60.0, 80.0, 100.0];
pub const GRID_TERM_BOOST_WEIGHTS: &[f32] = &[0.0, 0.01, 0.03, 0.05, 0.08, 0.1, 0.15, 0.2, 0.3];

// ── Stopwords list ──
pub fn get_stopwords() -> &'static std::collections::HashSet<String> {
    static STOPWORDS_CELL: std::sync::OnceLock<std::collections::HashSet<String>> = std::sync::OnceLock::new();
    STOPWORDS_CELL.get_or_init(|| {
        let mut set = std::collections::HashSet::new();
        // stop-words crate: Russian and English
        for w in stop_words::get(stop_words::LANGUAGE::English) {
            set.insert(w.to_lowercase());
        }
        for w in stop_words::get(stop_words::LANGUAGE::Russian) {
            set.insert(w.to_lowercase());
        }
        // extra custom Russian question/stop words
        let extra = [
            "такое", "это", "как", "почему", "зачем", "какой", "какая", "какие",
            "является", "означает", "значит", "дай", "расскажи", "объясни", "что"
        ];
        for w in &extra {
            set.insert(w.to_string());
        }
        set
    })
}
