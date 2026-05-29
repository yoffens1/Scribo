//! # Reranking Phase
//!
//! Post-fusion LLM-driven reranking with two strategies:
//! - **Scoring** — the LLM assigns each candidate a numeric relevance score.
//! - **Listwise** — the LLM returns a complete relevance permutation.

pub mod scoring;
pub mod listwise;

pub use scoring::rerank_scoring;
pub use listwise::rerank_listwise;
