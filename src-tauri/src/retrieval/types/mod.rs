//! # Retrieval Type Definitions
//!
//! Contains all shared types, enums and configuration structs used across the retrieval pipeline.
//! All public types are serialisable for Tauri IPC with camelCase field names.

pub mod config;
pub mod query;
pub mod result;

pub use config::{
    RetrievalMode, SynonymExpansion, RerankMode,
    PipelineConfig, AiRerankConfig, AdaptiveWeights, RetrievalTuning, RetrievalConfig,
};
pub use query::{VariantSource, QueryVariant};
pub use result::{
    FragmentRef, ScoreDebug, SearchResult,
    RetrieveFilters, RetrieveOptions,
    FetchQuery, FetchResult,
};
