

#[derive(Debug, Clone)]
pub struct FragmentOptions {
    pub max_tokens: usize,
    pub overlap_tokens: usize,
    pub preserve_tables: bool,
    pub lower_case: bool,
    pub remove_links: bool,
    pub remove_formatting: bool,
    pub format_latex: bool,
    pub linearize_tables: bool,
    pub fragment_by_headings: bool,
    pub include_heading_in_fragments: bool,
    pub remove_rules: bool,
    pub heading_level: usize,
    pub separate_sub_headings: bool,
    pub compact_lines: bool,
    pub remove_numbering: bool,
    pub strip_heading_markers: bool,
    pub keep_subheading_with_content: bool,
    pub remove_list_markers: bool,
    pub separate_tables_as_fragments: bool,
    pub each_table_row_as_separate_fragment: bool,
}

impl Default for FragmentOptions {
    fn default() -> Self {
        Self {
            lower_case: true,
            remove_links: true,
            remove_formatting: true,
            format_latex: true,
            remove_rules: true,
            remove_numbering: true,
            strip_heading_markers: true,
            remove_list_markers: true,
            compact_lines: true,
            fragment_by_headings: true,
            heading_level: 2,
            include_heading_in_fragments: true,
            separate_sub_headings: false,
            keep_subheading_with_content: true,
            preserve_tables: true,
            linearize_tables: true,
            each_table_row_as_separate_fragment: true,
            separate_tables_as_fragments: false,
            max_tokens: 256,
            overlap_tokens: 0,
        }
    }
}

pub enum FragmentMode {
    Embedding,
    Generation,
    Structural,
}

impl FragmentOptions {
    pub fn for_mode(&self, mode: FragmentMode) -> Self {
        match mode {
            FragmentMode::Embedding => Self {
                lower_case: true,
                remove_links: true,
                remove_formatting: true,
                format_latex: true,
                linearize_tables: true,
                fragment_by_headings: true,
                heading_level: 2,
                include_heading_in_fragments: true,
                separate_sub_headings: true,
                separate_tables_as_fragments: true,
                keep_subheading_with_content: true,
                remove_rules: true,
                compact_lines: true,
                remove_numbering: true,
                strip_heading_markers: true,
                remove_list_markers: true,
                each_table_row_as_separate_fragment: true,
                // Keep original values for non-preset fields
                max_tokens: self.max_tokens,
                overlap_tokens: self.overlap_tokens,
                preserve_tables: self.preserve_tables,
            },
            FragmentMode::Generation => Self {
                lower_case: true,
                remove_links: true,
                remove_formatting: true,
                format_latex: false,
                remove_rules: true,
                compact_lines: true,
                remove_numbering: true,
                strip_heading_markers: true,
                remove_list_markers: true,
                fragment_by_headings: true,
                heading_level: 2,
                include_heading_in_fragments: false,
                separate_sub_headings: true,
                keep_subheading_with_content: false,
                linearize_tables: false,
                separate_tables_as_fragments: true,
                preserve_tables: true,
                max_tokens: usize::MAX,
                overlap_tokens: 0,
                each_table_row_as_separate_fragment: self.each_table_row_as_separate_fragment,
            },
            FragmentMode::Structural => Self {
                lower_case: false,
                remove_links: false,
                remove_formatting: false,
                format_latex: false,
                remove_rules: false,
                remove_numbering: false,
                strip_heading_markers: false,
                remove_list_markers: false,
                compact_lines: false,
                fragment_by_headings: self.fragment_by_headings,
                heading_level: self.heading_level,
                include_heading_in_fragments: self.include_heading_in_fragments,
                separate_sub_headings: self.separate_sub_headings,
                keep_subheading_with_content: self.keep_subheading_with_content,
                preserve_tables: self.preserve_tables,
                separate_tables_as_fragments: self.separate_tables_as_fragments,
                linearize_tables: self.linearize_tables,
                each_table_row_as_separate_fragment: self.each_table_row_as_separate_fragment,
                max_tokens: self.max_tokens,
                overlap_tokens: self.overlap_tokens,
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct TableInfo {
    pub placeholder: String,
    pub content: String,
    pub tokens: usize,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FragmenterPair {
    pub embedding: String,
    pub generation: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FragmenterResult {
    pub pairs: Vec<FragmenterPair>,
    pub metadata: Option<serde_json::Map<String, serde_json::Value>>,
}
