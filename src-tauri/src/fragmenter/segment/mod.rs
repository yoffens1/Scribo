//! # Segment Module
//!
//! Segmenters split a markdown document into **Atoms** — the smallest structural units
//! that the packer then aggregates into final fragments.
//!
//! ## Available Segmenters
//!
//! | Module | Configured by | Characteristics |
//! |---|---|---|
//! | [`heading`] | `Segmenter::HeadingSections` | Splits at heading boundaries — semantically clean, structure-preserving |
//! | [`block`]   | `Segmenter::Blocks`          | Splits at every `pulldown-cmark` block event — fine-grained, layout-aware |
//! | [`semantic`]| `Segmenter::Semantic`        | Block-splits then merges similar blocks via embedding cosine similarity |

use std::ops::Range;

pub mod heading;
pub mod block;
pub mod semantic;

// ─── AtomKind ─────────────────────────────────────────────────────────────────

/// The structural type of a markdown block, used by packers and the pipeline
/// to make layout-aware merging decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtomKind {
    Heading { level: u8 },
    Paragraph,
    List,
    CodeBlock,
    Table,
    Other,
}

// ─── Atom ─────────────────────────────────────────────────────────────────────

/// The finest-grained structural unit produced by a segmenter.
/// One or more Atoms are merged into a [`RawFragment`](crate::fragmenter::pack::RawFragment)
/// by the configured [`Packer`](crate::fragmenter::config::Packer).
#[derive(Debug, Clone)]
pub struct Atom {
    /// The raw markdown text of this block (no cleaning applied yet).
    pub text: String,
    /// The type of markdown block this atom represents.
    pub kind: AtomKind,
    /// Byte range of this atom in the original source document.
    pub range: Option<Range<usize>>,
    /// Full heading ancestry path, e.g. `["Chapter 1", "Section 1.2"]`.
    pub heading_path: Vec<String>,
    /// The most immediate heading above this atom, used as `suggested_title` in meta.
    pub heading_title: Option<String>,
    /// `true` for atoms directly under a top-level heading (H1/H2).
    pub is_top_level: bool,
    /// Estimated token count of `text`.
    pub token_count: usize,
    /// Byte length of `text`.
    pub char_count: usize,
}

impl Atom {
    /// Creates a single whole-document atom covering all of `content`.
    /// Used by [`Segmenter::Whole`](crate::fragmenter::config::Segmenter::Whole).
    pub fn whole(content: &str) -> Self {
        Self {
            text: content.to_string(),
            kind: AtomKind::Other,
            range: Some(0..content.len()),
            heading_path: Vec::new(),
            heading_title: None,
            is_top_level: false,
            token_count: crate::fragmenter::token::count_tokens(content),
            char_count: content.len(),
        }
    }
}
