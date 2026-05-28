#[derive(Debug, Clone)]
pub enum Segmenter {
    /// Splits by markdown headings of level N.
    HeadingSections {
        level: usize,                       // 1..=6
        separate_sub_headings: bool,
        keep_subheading_with_content: bool,
    },
    /// Splits by pulldown_cmark blocks (paragraphs, lists, codeblocks, headings...).
    Blocks {
        flush_on_top_heading: bool,         // true = new chunk on each H1/H2
    },
    /// Blocks + merging them using cosine similarity between consecutive blocks.
    Semantic {
        max_chars: usize,                   // max chars limit per semantic topic block
        threshold: f32,
    },
    /// No segmentation — the whole document is treated as a single segment.
    Whole,
}

#[derive(Debug, Clone)]
pub enum Packer {
    /// Aggregates segments into a target token budget.
    TokenBudget {
        max_tokens: usize,
        overlap_tokens: usize,
    },
    /// Aggregates segments into a target character budget (used for topics).
    CharBudget {
        max_chars: usize,
    },
    /// Passes segments through as-is without aggregation.
    Passthrough,
}

#[derive(Debug, Clone)]
pub enum CleanProfile {
    /// Raw markdown without transformations.
    Raw,
    /// Preset for embeddings: lower-case, remove links/formatting, format latex, linearize tables.
    Embedding,
    /// Preset for generation: keep case/latex, clean rules/list markers.
    Generation,
    /// Custom flags.
    Custom(CleanFlags),
}

#[derive(Debug, Clone, Default)]
pub struct CleanFlags {
    pub lower_case: bool,
    pub remove_links: bool,
    pub remove_formatting: bool,
    pub format_latex: bool,
    pub remove_rules: bool,
    pub remove_numbering: bool,
    pub remove_list_markers: bool,
    pub strip_heading_markers: bool,
    pub compact_lines: bool,
    // table options
    pub preserve_tables: bool,
    pub linearize_tables: bool,
    pub separate_tables_as_fragments: bool,
    pub each_table_row_as_separate_fragment: bool,
}

impl CleanProfile {
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
                format_latex: true,
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
                format_latex: false,
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

#[derive(Debug, Clone)]
pub struct FragmentConfig {
    pub segmenter: Segmenter,
    pub packer: Packer,
    pub cleaner: CleanProfile,
    pub include_heading_in_fragments: bool,
    pub extract_frontmatter: bool,
}

impl Default for FragmentConfig {
    fn default() -> Self {
        Self::embedding()
    }
}

impl FragmentConfig {
    /// Legacy fragment_for_embedding preset.
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

    /// Legacy fragment_for_generation preset.
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

    /// Legacy RuleChunker for distribute.
    pub fn topic(max_chars: usize) -> Self {
        Self {
            segmenter: Segmenter::Blocks { flush_on_top_heading: true },
            packer: Packer::CharBudget { max_chars },
            cleaner: CleanProfile::Raw,
            include_heading_in_fragments: false,
            extract_frontmatter: true,
        }
    }

    /// Legacy SemanticChunker for distribute.
    pub fn topic_semantic(max_chars: usize, threshold: f32) -> Self {
        Self {
            segmenter: Segmenter::Semantic { max_chars, threshold },
            packer: Packer::CharBudget { max_chars },
            cleaner: CleanProfile::Raw,
            include_heading_in_fragments: false,
            extract_frontmatter: true,
        }
    }

    /// Structural mode for domain mappings.
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

    pub fn with_packer(mut self, p: Packer) -> Self {
        self.packer = p;
        self
    }

    pub fn with_cleaner(mut self, c: CleanProfile) -> Self {
        self.cleaner = c;
        self
    }

    pub fn with_segmenter(mut self, s: Segmenter) -> Self {
        self.segmenter = s;
        self
    }
}
