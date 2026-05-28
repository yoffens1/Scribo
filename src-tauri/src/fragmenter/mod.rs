//! # Fragmenter Module
//!
//! Splits a Markdown document into **fragments** — self-contained text chunks ready for
//! embedding or LLM generation. The pipeline is driven by three orthogonal axes:
//!
//! ```text
//!  Markdown source
//!       │
//!       ▼
//!  ┌─────────────┐   extracts YAML ---...--- blocks
//!  │ Frontmatter │──► metadata map
//!  └─────────────┘
//!       │ remaining text
//!       ▼
//!  ┌─────────────┐   lifts <table> blocks out of the stream
//!  │   Tables    │──► (table_id, TableInfo)  ← restored later
//!  └─────────────┘
//!       │ body text (tables replaced by placeholders)
//!       ▼
//!  ┌─────────────────────────────────────────────────────────┐
//!  │                     Segmenter                           │
//!  │  HeadingSections │ Blocks │ Semantic │ Whole            │
//!  └─────────────────────────────────────────────────────────┘
//!       │ Vec<Atom>  (fine-grained structural units)
//!       ▼
//!  ┌──────────────────────────────────────────────────────────┐
//!  │                      Packer                              │
//!  │  TokenBudget │ CharBudget │ Passthrough                  │
//!  └──────────────────────────────────────────────────────────┘
//!       │ Vec<RawFragment>
//!       ▼  table placeholders → real TableInfo
//!  ┌─────────────┐
//!  │  Restore    │  (tables spliced back in, optionally linearized)
//!  └─────────────┘
//!       │
//!       ▼
//!  ┌────────────────┐
//!  │ CleanProfile   │  Raw | Embedding | Generation | Custom
//!  └────────────────┘
//!       │ Vec<Fragment>  (text + FragmentMeta)
//!       ▼
//!   caller / indexer / SRS card generator
//! ```
//!
//! ## Quick-start
//!
//! ```rust,ignore
//! // For embedding (lower-case, no markdown syntax, linearized tables):
//! let texts = fragment_for_embedding(content, &FragmentConfig::default());
//!
//! // For SRS generation (preserves headings, inline LaTeX, tables):
//! let texts = fragment_for_generation(content, &FragmentConfig::generation());
//!
//! // Paired — produces (embedding_text, generation_text) tuples from the same segmentation:
//! let pairs = fragment_paired(content.to_string(), &FragmentConfig::default());
//! ```
//!
//! ## Modules
//!
//! - [`config`]   — `FragmentConfig`, segmenter/packer/cleaner enums and presets.
//! - [`output`]   — `Fragment`, `FragmentMeta`, `FragmenterResult`, `FragmenterPair`.
//! - [`pipeline`] — `Fragmenter` struct driving the full pipeline.
//! - [`segment`]  — Segmenters: `heading`, `block`, `semantic`.
//! - [`pack`]     — Packers: `token_budget`, `char_budget`, `passthrough`, `tables`.
//! - [`clean`]    — Text cleaners: `markdown`, `latex`, `tables`, `apply`.
//! - [`token`]    — Token counting via `tiktoken-rs`.

pub mod config;
pub mod output;
pub mod pipeline;
pub mod segment;
pub mod pack;
pub mod clean;
pub mod token;

pub use config::{FragmentConfig, Segmenter, Packer, CleanProfile, CleanFlags, LatexHandling};
pub use output::{Fragment, FragmentMeta, FragmenterResult, FragmenterPair, FragmenterPairedResult};
pub use pipeline::Fragmenter;

// Backward-compatibility aliases so callers don't need to be updated.
pub type FragmentOptions = FragmentConfig;
pub type FragmentMode = CleanProfile;
pub use clean::tables::TableInfo;

/// Convenience wrapper: runs the paired pipeline synchronously.
/// Returns `(embedding_text, generation_text)` tuples for every fragment.
pub fn fragment_paired(content: String, options: &FragmentConfig) -> FragmenterPairedResult {
    let fragmenter = Fragmenter::new();
    fragmenter.run_paired_sync(&content, options)
}

/// Convenience wrapper: fragment content with the `Embedding` clean profile.
/// Returns only the cleaned text strings — suitable for feeding directly into an embedder.
pub fn fragment_for_embedding(content: &str, options: &FragmentConfig) -> Vec<String> {
    let fragmenter = Fragmenter::new();
    let config = FragmentConfig {
        segmenter: options.segmenter.clone(),
        packer: options.packer.clone(),
        cleaner: CleanProfile::Embedding,
        include_heading_in_fragments: options.include_heading_in_fragments,
        extract_frontmatter: options.extract_frontmatter,
    };
    let res = fragmenter.run_sync(content, &config);
    res.fragments.into_iter().map(|f| f.text).collect()
}

/// Convenience wrapper: fragment content with the `Generation` clean profile.
/// Returns cleaned text strings suitable for feeding into a generation LLM.
pub fn fragment_for_generation(content: &str, options: &FragmentConfig) -> Vec<String> {
    let fragmenter = Fragmenter::new();
    let config = FragmentConfig {
        segmenter: options.segmenter.clone(),
        packer: options.packer.clone(),
        cleaner: CleanProfile::Generation,
        include_heading_in_fragments: options.include_heading_in_fragments,
        extract_frontmatter: options.extract_frontmatter,
    };
    let res = fragmenter.run_sync(content, &config);
    res.fragments.into_iter().map(|f| f.text).collect()
}

// ─── Chunker trait & impls (used by the distribute pipeline) ─────────────────

/// Abstraction over chunking strategies for the `distribute` pipeline.
/// A `Chunker` converts a raw markdown draft into [`TopicChunk`](crate::domain::distribute::TopicChunk)s
/// that are then matched against target notes.
#[async_trait::async_trait]
pub trait Chunker {
    async fn chunk(
        &self,
        content: &str,
        llm_service: &std::sync::Arc<crate::ai::LlmService>,
    ) -> Vec<crate::domain::distribute::TopicChunk>;
}

/// Rule-based chunker: splits by block boundaries (`Segmenter::Blocks`)
/// and packs into character budget windows. No LLM required.
pub struct RuleChunker {
    pub max_chars: usize,
}

impl RuleChunker {
    pub fn new(max_chars: usize) -> Self {
        Self { max_chars }
    }
}

#[async_trait::async_trait]
impl Chunker for RuleChunker {
    async fn chunk(
        &self,
        content: &str,
        _llm_service: &std::sync::Arc<crate::ai::LlmService>,
    ) -> Vec<crate::domain::distribute::TopicChunk> {
        let config = FragmentConfig::topic(self.max_chars);
        let fragmenter = Fragmenter::new();
        let result = fragmenter.run(content, &config).await;
        result
            .fragments
            .into_iter()
            .map(|f| crate::domain::distribute::TopicChunk {
                text: f.text,
                suggested_title: f.meta.suggested_title.unwrap_or_default(),
            })
            .collect()
    }
}

/// Semantic chunker: generates embeddings for each block and merges consecutive blocks
/// that are semantically similar (cosine similarity ≥ `threshold`).
/// Requires an LLM service for embedding generation.
pub struct SemanticChunker {
    pub max_chars: usize,
    /// Cosine similarity threshold for merging two consecutive blocks.
    /// Values closer to 1.0 produce finer-grained chunks; lower values merge more aggressively.
    pub threshold: f32,
}

impl SemanticChunker {
    pub fn new(max_chars: usize, threshold: f32) -> Self {
        Self { max_chars, threshold }
    }
}

#[async_trait::async_trait]
impl Chunker for SemanticChunker {
    async fn chunk(
        &self,
        content: &str,
        llm_service: &std::sync::Arc<crate::ai::LlmService>,
    ) -> Vec<crate::domain::distribute::TopicChunk> {
        let config = FragmentConfig::topic_semantic(self.max_chars, self.threshold);
        let fragmenter = Fragmenter::with_llm(llm_service.clone());
        let result = fragmenter.run(content, &config).await;
        result
            .fragments
            .into_iter()
            .map(|f| crate::domain::distribute::TopicChunk {
                text: f.text,
                suggested_title: f.meta.suggested_title.unwrap_or_default(),
            })
            .collect()
    }
}
