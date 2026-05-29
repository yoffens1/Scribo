//! # Retrieval Module
//!
//! Implements the full hybrid retrieval stack used by search and RAG operations.
//!
//! ## Architecture
//!
//! ```text
//!  Query ──► preprocess::build_variants
//!              │
//!              ├─ [Original]
//!              ├─ [Translated?]      (preprocess/translate.rs)
//!              ├─ [HyDE?]            (preprocess/hyde.rs)
//!              └─ [Synonym?...]      (preprocess/synonyms.rs)
//!                          │
//!              search::build_variant_embeddings  (1 batched LLM call)
//!                          │
//!         ┌────────────────┤ (concurrent, spawn_blocking per variant)
//!         ▼                ▼
//!    FTS5 keyword    cosine vector
//!    search (BM25)   scan (ANN)
//!         └────────────────┤
//!               search::fusion::rrf() per variant
//!                          │
//!           pipeline::rrf() across all variants
//!                          │
//!                apply_filters()
//!                          │
//!             rerank::rerank_*() (LLM)
//!                          │
//!                    top_k results
//! ```
//!
//! ## Modules
//!
//! - [`types`]      — All public types: configs, enums, query/result structs.
//! - [`config`]     — Config resolver (loads calibrated DB settings, resolves LLM model).
//! - [`preprocess`] — Query preprocessing: variants, translation, HyDE, synonyms.
//! - [`search`]     — Search execution: FTS5 + vector, RRF fusion, vault language detection.
//! - [`rerank`]     — Post-fusion LLM reranking: scoring and listwise.
//! - [`pipeline`]   — Top-level orchestrator: `retrieve()`, `fetch()`, `embed_query()`.
//! - [`cache`]      — Cross-cutting LLM response cache.
//! - [`calibration`]— Offline parameter tuning (grid search, MRR).

pub mod types;
pub mod config;
pub mod preprocess;
pub mod search;
pub mod rerank;
pub mod pipeline;
pub mod config_resolver;
pub mod cache;
pub mod context;
pub mod calibration;

pub use types::{RetrievalConfig, SearchResult, RetrieveOptions, RetrieveFilters, FetchQuery, FetchResult};
pub use pipeline::{retrieve, fetch, embed_query};
pub use calibration::{EvalSample, mean_reciprocal_rank, grid_search, calibrate_min_score};

