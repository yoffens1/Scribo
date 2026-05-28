//! # Distribute Service
//!
//! The distribution pipeline converts a freeform draft note into structured, indexed notes
//! by routing each semantic topic chunk to the most relevant existing note (or creating one).
//!
//! ## Pipeline overview
//!
//! ```text
//!  Draft note (raw markdown)
//!       │
//!       ▼  SemanticChunker / RuleChunker
//!  Vec<TopicChunk>  (semantically coherent sections)
//!       │
//!       ▼  VectorRetriever  (parallel, one query per chunk)
//!  Vec<CandidateNote>  (top-3 similar existing notes per chunk)
//!       │
//!       ▼  LLM batch prompt  (analyze.rs)
//!  Vec<LlmRecommendation>  (Append / CreateChild / Skip per chunk)
//!       │
//!       ▼  HeuristicClassifier  (classifier.rs)
//!  Post-processed plan with inferred parent-child links
//!       │
//!       ▼  apply_distribution  (apply.rs)
//!  Mutations committed to the DB (append text, create notes, etc.)
//! ```
//!
//! ## Modules
//!
//! - [`analyze`]        — orchestrates chunking + retrieval + LLM batch recommendation.
//! - [`classifier`]     — heuristic post-processing to infer parent-child links between new notes.
//! - [`candidate_search`] — embeds each chunk and finds candidate target notes via vector search.
//! - [`apply`]          — commits the approved `DraftDistributionPlan` to the database.
//! - [`refresh_cards`]  — regenerates stale SRS cards for notes modified by distribution.

pub mod classifier;
pub mod candidate_search;
pub mod apply;
pub mod refresh_cards;
pub mod analyze;

pub use crate::fragmenter::{Chunker, RuleChunker, SemanticChunker};
pub use classifier::{Classifier, HeuristicClassifier, apply_heuristic_linking};
pub use candidate_search::{Retriever, VectorRetriever};
pub use apply::apply_distribution;
pub use refresh_cards::refresh_stale_cards_for_notes;
pub use analyze::analyze_draft_for_distribution;
