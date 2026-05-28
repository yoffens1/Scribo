//! # Fragmenter Configuration
//!
//! Defines the three orthogonal axes of the fragmenter pipeline:
//!
//! | Axis | Type | Controls |
//! |---|---|---|
//! | Segmentation | [`Segmenter`] | *How* the markdown is split into atoms |
//! | Packing | [`Packer`] | *How many* atoms are merged into one fragment |
//! | Cleaning | [`CleanProfile`] | *What* transformations are applied to the text |
//!
//! Use the named presets on [`FragmentConfig`] rather than constructing it by hand.

// ─── Segmenter ────────────────────────────────────────────────────────────────

/// Determines the granularity at which the document is split into [`Atom`](crate::fragmenter::segment::Atom)s.
///
/// Atoms are the finest-grained unit — they are subsequently merged by the [`Packer`].
#[derive(Debug, Clone)]
pub enum Segmenter {
    /// Splits the document at markdown headings of the given `level`.
    /// Each heading and its associated content form one atom.
    ///
    /// - `level` — heading depth to split at (1–6).
    /// - `separate_sub_headings` — if `true`, sub-headings within a section create new atoms.
    /// - `keep_subheading_with_content` — if `true`, a sub-heading atom includes the text that follows it.
    HeadingSections {
        level: usize,
        separate_sub_headings: bool,
        keep_subheading_with_content: bool,
    },

    /// Splits using `pulldown-cmark` block events: paragraphs, list items, code blocks, headings, etc.
    ///
    /// - `flush_on_top_heading` — when `true`, every H1/H2 forces a new fragment boundary,
    ///   useful for topic-level chunking in the distribute pipeline.
    Blocks {
        flush_on_top_heading: bool,
    },

    /// Runs `Blocks` segmentation first, then merges consecutive blocks whose embedding vectors
    /// are closer than `threshold` (cosine similarity). Requires an LLM service to generate embeddings.
    ///
    /// - `max_chars` — hard cap: a merged group never exceeds this many characters.
    /// - `threshold` — merging threshold in [0, 1]. Higher = finer-grained chunks.
    Semantic {
        max_chars: usize,
        threshold: f32,
    },

    /// Treats the entire document as a single atom — no splitting at all.
    /// Useful for short notes or when the caller wants full-document embeddings.
    Whole,
}

// ─── Packer ───────────────────────────────────────────────────────────────────

/// Determines how atoms are aggregated into final fragments.
#[derive(Debug, Clone)]
pub enum Packer {
    /// Greedy window packer: accumulates atoms until the combined token count exceeds `max_tokens`,
    /// then starts a new fragment. Consecutive fragments may share `overlap_tokens` of context.
    TokenBudget {
        max_tokens: usize,
        overlap_tokens: usize,
    },

    /// Same greedy logic but measured in *characters* rather than tokens.
    /// Used by the distribute pipeline where exact LLM token counts are not critical.
    CharBudget {
        max_chars: usize,
    },

    /// Emits every atom as its own fragment without any merging.
    /// Useful when the segmenter already produces the desired granularity (e.g. `HeadingSections`
    /// with the `Generation` profile).
    Passthrough,
}

// ─── LaTeX handling ───────────────────────────────────────────────────────────

/// Controls how inline and display LaTeX expressions are handled during cleaning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LatexHandling {
    /// Leave LaTeX delimiters and content unchanged (`$...$`, `$$...$$`).
    Keep,
    /// Convert LaTeX to a human-readable linearised form (e.g. `x^2` → `x squared`).
    Format,
    /// Strip all LaTeX expressions entirely.
    Remove,
}

impl Default for LatexHandling {
    fn default() -> Self {
        LatexHandling::Keep
    }
}

// ─── Clean profiles ───────────────────────────────────────────────────────────

/// High-level preset for text cleaning. Resolves to a [`CleanFlags`] bitset.
#[derive(Debug, Clone)]
pub enum CleanProfile {
    /// No transformations — raw markdown output.
    Raw,
    /// Optimised for embedding models:
    /// lower-case, strip markdown syntax, format LaTeX to readable text, linearise tables.
    Embedding,
    /// Optimised for LLM generation / SRS card creation:
    /// lower-case, strip syntax, preserve LaTeX, keep tables structured.
    Generation,
    /// Fully custom flags for callers that need fine-grained control.
    Custom(CleanFlags),
}

/// Granular boolean flags that control each individual cleaning step.
/// Prefer [`CleanProfile`] presets over constructing this directly.
#[derive(Debug, Clone, Default)]
pub struct CleanFlags {
    /// Convert all text to lower-case.
    pub lower_case: bool,
    /// Remove `[text](url)` / `[[wikilink]]` syntax (keep only the label).
    pub remove_links: bool,
    /// Strip bold/italic/code-span markers (`**`, `*`, `` ` ``).
    pub remove_formatting: bool,
    /// How to handle LaTeX math expressions.
    pub latex: LatexHandling,
    /// Remove horizontal rules (`---`, `***`).
    pub remove_rules: bool,
    /// Remove ordered-list numbering (`1.`, `2.` …).
    pub remove_numbering: bool,
    /// Remove unordered-list markers (`-`, `*`, `+`).
    pub remove_list_markers: bool,
    /// Strip leading `#` heading markers.
    pub strip_heading_markers: bool,
    /// Collapse multiple consecutive blank lines into one.
    pub compact_lines: bool,

    // ── Table options ──────────────────────────────────────────────────────────
    /// Keep pipe-table syntax in the output (tables are extracted then re-inserted).
    pub preserve_tables: bool,
    /// Replace pipe-table syntax with a flat prose representation for embedding.
    pub linearize_tables: bool,
    /// Emit each table as a separate fragment instead of inlining it.
    pub separate_tables_as_fragments: bool,
    /// When splitting tables, treat every row as its own fragment.
    pub each_table_row_as_separate_fragment: bool,
}

impl CleanProfile {
    /// Resolves the profile to concrete [`CleanFlags`] used by [`clean::apply`](crate::fragmenter::clean::apply).
    pub fn to_flags(&self) -> CleanFlags {
        match self {
            CleanProfile::Raw => CleanFlags {
                preserve_tables: true,
                ..Default::default()
            },
            CleanProfile::Embedding => CleanFlags {
                lower_case: true,
                remove_links: true,
                remove_formatting: true,
                latex: LatexHandling::Format,
                remove_rules: true,
                remove_numbering: true,
                remove_list_markers: true,
                strip_heading_markers: true,
                compact_lines: true,
                preserve_tables: true,
                linearize_tables: true,
                separate_tables_as_fragments: true,
                each_table_row_as_separate_fragment: true,
            },
            CleanProfile::Generation => CleanFlags {
                lower_case: true,
                remove_links: true,
                remove_formatting: true,
                latex: LatexHandling::Keep,
                remove_rules: true,
                remove_numbering: true,
                remove_list_markers: true,
                strip_heading_markers: true,
                compact_lines: true,
                preserve_tables: true,
                linearize_tables: false,
                separate_tables_as_fragments: true,
                each_table_row_as_separate_fragment: false,
            },
            CleanProfile::Custom(flags) => flags.clone(),
        }
    }
}

// ─── FragmentConfig ───────────────────────────────────────────────────────────

/// The single configuration object passed to [`Fragmenter::run`](crate::fragmenter::pipeline::Fragmenter::run).
/// Use the named constructors instead of filling every field manually.
#[derive(Debug, Clone)]
pub struct FragmentConfig {
    /// How to split the document into atoms.
    pub segmenter: Segmenter,
    /// How to aggregate atoms into fragments.
    pub packer: Packer,
    /// How to clean the fragment text.
    pub cleaner: CleanProfile,
    /// If `true`, the section heading is prepended to each fragment's text,
    /// so the embedding model has the full context of the passage.
    pub include_heading_in_fragments: bool,
    /// If `true`, YAML frontmatter (`---...---`) is parsed and returned as metadata.
    pub extract_frontmatter: bool,
}

impl Default for FragmentConfig {
    fn default() -> Self {
        Self::embedding()
    }
}

impl FragmentConfig {
    /// Preset for indexing notes into the vector store.
    /// H2-based sections → 256-token windows → Embedding cleaning.
    pub fn embedding() -> Self {
        Self {
            segmenter: Segmenter::HeadingSections {
                level: 2,
                separate_sub_headings: true,
                keep_subheading_with_content: true,
            },
            packer: Packer::TokenBudget {
                max_tokens: 256,
                overlap_tokens: 0,
            },
            cleaner: CleanProfile::Embedding,
            include_heading_in_fragments: true,
            extract_frontmatter: true,
        }
    }

    /// Preset for generating SRS flash-cards.
    /// H2-based sections → passthrough (one fragment per section) → Generation cleaning.
    pub fn generation() -> Self {
        Self {
            segmenter: Segmenter::HeadingSections {
                level: 2,
                separate_sub_headings: true,
                keep_subheading_with_content: false,
            },
            packer: Packer::Passthrough,
            cleaner: CleanProfile::Generation,
            include_heading_in_fragments: false,
            extract_frontmatter: true,
        }
    }

    /// Preset for the distribute pipeline's rule-based chunker.
    /// Splits on block boundaries, packs into character windows, returns raw markdown.
    pub fn topic(max_chars: usize) -> Self {
        Self {
            segmenter: Segmenter::Blocks { flush_on_top_heading: true },
            packer: Packer::CharBudget { max_chars },
            cleaner: CleanProfile::Raw,
            include_heading_in_fragments: false,
            extract_frontmatter: true,
        }
    }

    /// Preset for the distribute pipeline's semantic chunker.
    /// Merges blocks by embedding similarity up to `threshold`, packs into character windows.
    pub fn topic_semantic(max_chars: usize, threshold: f32) -> Self {
        Self {
            segmenter: Segmenter::Semantic { max_chars, threshold },
            packer: Packer::CharBudget { max_chars },
            cleaner: CleanProfile::Raw,
            include_heading_in_fragments: false,
            extract_frontmatter: true,
        }
    }

    /// Preset for structural mapping (domain taxonomy, note-tree analysis).
    /// H2 sections, passthrough packer, raw cleaning — preserves all markdown.
    pub fn structural() -> Self {
        Self {
            segmenter: Segmenter::HeadingSections {
                level: 2,
                separate_sub_headings: false,
                keep_subheading_with_content: true,
            },
            packer: Packer::Passthrough,
            cleaner: CleanProfile::Raw,
            include_heading_in_fragments: true,
            extract_frontmatter: true,
        }
    }

    // ── Builder helpers ────────────────────────────────────────────────────────

    /// Override the packer while keeping all other settings.
    pub fn with_packer(mut self, p: Packer) -> Self {
        self.packer = p;
        self
    }

    /// Override the cleaner profile while keeping all other settings.
    pub fn with_cleaner(mut self, c: CleanProfile) -> Self {
        self.cleaner = c;
        self
    }

    /// Override the segmenter while keeping all other settings.
    pub fn with_segmenter(mut self, s: Segmenter) -> Self {
        self.segmenter = s;
        self
    }
}
