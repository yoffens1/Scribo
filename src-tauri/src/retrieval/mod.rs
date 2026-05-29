//! # Retrieval Module
//!
//! Implements the full hybrid retrieval stack used by search and RAG operations.
//!
//! ## Architecture
//!
//! ```text
//!  Query ──► build_variants ──► [Original, Translated?, HyDE?, Synonym?...]
//!                                         │
//!                          build_variant_embeddings  (1 batched LLM call)
//!                                         │
//!                    ┌────────────────────┤ (concurrent, spawn_blocking)
//!                    ▼                    ▼
//!               FTS5 keyword        cosine vector
//!               search (BM25)       scan (ANN)
//!                    └────────────────────┤
//!                                  rrf() per variant
//!                                         │
//!                            rrf() across all variants
//!                                         │
//!                               apply_filters()
//!                                         │
//!                                rerank() (LLM)
//!                                         │
//!                                    top_k results
//! ```
//!
//! ## Modules
//!
//! - [`language`]  — `whatlang`-backed language detection and ISO-639-1 mapping.
//! - [`types`]     — All public types: configs, enums, query/result structs.
//! - [`fusion`]    — Reciprocal Rank Fusion (RRF) algorithm for score merging.
//! - [`stages`]    — Preprocessing stages: translation, HyDE, synonym expansion.
//! - [`rerankers`] — Post-fusion LLM reranking: scoring and listwise.
//! - [`pipeline`]  — Central orchestrator that drives the full retrieval flow.

pub mod types;
pub mod fusion;
pub mod stages;
pub mod rerankers;
pub mod pipeline;
pub mod config_resolver;
pub mod cache;
pub mod context;
pub mod calibration;



pub use types::{RetrievalConfig, SearchResult, RetrieveOptions, RetrieveFilters, FetchQuery, FetchResult};
pub use pipeline::{retrieve, fetch, embed_query};
pub use calibration::{EvalSample, mean_reciprocal_rank, grid_search, calibrate_min_score};
