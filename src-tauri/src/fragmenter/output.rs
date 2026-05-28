//! # Fragmenter Output Types
//!
//! Defines the data structures that flow *out* of the fragmenter pipeline.
//! These types are serialisable for Tauri IPC (camelCase JSON field names).

use std::ops::Range;

// ─── Fragment ─────────────────────────────────────────────────────────────────

/// A single text chunk produced by the fragmenter.
/// Contains the cleaned text and structural metadata.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Fragment {
    /// The processed text of this fragment (cleaning applied according to the [`CleanProfile`](crate::fragmenter::config::CleanProfile)).
    pub text: String,
    /// Structural information about where this fragment came from.
    pub meta: FragmentMeta,
}

/// Structural metadata attached to every [`Fragment`].
#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FragmentMeta {
    /// 0-based sequential index within the result set.
    pub index: usize,
    /// Byte range in the *original* source document this fragment was derived from.
    pub source_range: Option<Range<usize>>,
    /// Ordered list of heading titles tracing from the document root to this fragment's section.
    /// E.g. `["Introduction", "Background", "Prior Work"]`.
    pub heading_path: Vec<String>,
    /// The most immediate section heading above this fragment, used as a display title.
    pub suggested_title: Option<String>,
    /// `true` if this fragment corresponds to a top-level section (H1 or H2 heading).
    pub is_top_level_section: bool,
    /// Estimated token count of `text` (tiktoken cl100k_base).
    pub token_count: usize,
    /// Byte length of `text` in UTF-8.
    pub char_count: usize,
}

// ─── FragmenterResult ─────────────────────────────────────────────────────────

/// The result of a single [`Fragmenter::run`](crate::fragmenter::pipeline::Fragmenter::run) call.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FragmenterResult {
    pub fragments: Vec<Fragment>,
    /// Parsed YAML frontmatter of the source document, if any and `extract_frontmatter` was enabled.
    pub metadata: Option<serde_json::Map<String, serde_json::Value>>,
}

// ─── Paired types (embedding + generation) ────────────────────────────────────

/// A matched pair of `(embedding_text, generation_text)` derived from the same source fragment.
///
/// - `embedding` — cleaned for vector indexing (lower-case, no markdown, linearised tables).
/// - `generation` — cleaned for LLM prompts (preserves LaTeX, table structure).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FragmenterPair {
    pub embedding: String,
    pub generation: String,
}

/// The result of [`Fragmenter::run_paired`](crate::fragmenter::pipeline::Fragmenter::run_paired).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FragmenterPairedResult {
    pub pairs: Vec<FragmenterPair>,
    /// Parsed YAML frontmatter, same as [`FragmenterResult::metadata`].
    pub metadata: Option<serde_json::Map<String, serde_json::Value>>,
}
