use std::sync::LazyLock;
use rayon::prelude::*;
use crate::fragmenter::stages::extract;
use crate::fragmenter::types::FragmentOptions;
use crate::fragmenter::markdown::table;
use crate::fragmenter::stages::token;
use super::assemble::{glue_subheadings_to_content, assemble_raw_fragments};
use super::table_restore::{restore_tables, linearize_table_fragments};
use super::sub_headings::split_fragments_by_sub_headings;
use super::clean::clean_fragment;
use super::headings::prepend_heading_to_fragments;

static RE_PARA: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"\n\s*\n").unwrap());
static RE_HEADING_LEVELS: LazyLock<[regex::Regex; 6]> = LazyLock::new(|| [
    regex::Regex::new(r"^#{1,6}\s").unwrap(),
    regex::Regex::new(r"^#{2,6}\s").unwrap(),
    regex::Regex::new(r"^#{3,6}\s").unwrap(),
    regex::Regex::new(r"^#{4,6}\s").unwrap(),
    regex::Regex::new(r"^#{5,6}\s").unwrap(),
    regex::Regex::new(r"^#{6,6}\s").unwrap(),
]);

pub fn fragment_by_heading_sections(content: &str, options: &FragmentOptions) -> Vec<String> {
    let sections = extract::split_by_headings(content, options.heading_level);
    sections
        .par_iter()
        .flat_map(|section| {
            let section_heading = extract_section_heading(section, options);
            let target_heading = if options.include_heading_in_fragments {
                section_heading
            } else {
                None
            };
            process_section(section, options, target_heading.as_deref())
        })
        .collect()
}

pub fn extract_section_heading(section: &str, options: &FragmentOptions) -> Option<String> {
    let first_line = section.trim_start().lines().next()?.trim();
    let idx = options.heading_level.saturating_sub(1).min(5);
    let re = &RE_HEADING_LEVELS[idx];
    if re.is_match(first_line) {
        Some(first_line.to_string())
    } else {
        None
    }
}

pub fn process_section(text: &str, options: &FragmentOptions, section_heading: Option<&str>) -> Vec<String> {
    let (body_text, tables) = if options.preserve_tables {
        table::extract_tables(text)
    } else {
        (text.to_string(), Vec::new())
    };

    let paragraphs: Vec<&str> = RE_PARA
        .split(&body_text)
        .filter(|p| !p.trim().is_empty())
        .collect();

    let paragraphs_cow = if options.keep_subheading_with_content {
        glue_subheadings_to_content(paragraphs)
    } else {
        paragraphs.into_iter().map(std::borrow::Cow::Borrowed).collect()
    };

    let raw_fragments = assemble_raw_fragments(paragraphs_cow, options);
    let mut merged_fragments = restore_tables(raw_fragments, &tables, options);

    if options.separate_sub_headings {
        merged_fragments = split_fragments_by_sub_headings(merged_fragments, options.heading_level);
    }

    merged_fragments = linearize_table_fragments(merged_fragments, options);

    let mut processed: Vec<String> = merged_fragments
        .iter()
        .map(|fragment| clean_fragment(fragment, options))
        .filter(|c| !c.is_empty())
        .collect();

    if options.max_tokens > 0 {
        processed = processed
            .into_iter()
            .flat_map(|fragment| {
                if token::count_tokens(&fragment) > options.max_tokens {
                    token::split_oversized_paragraph(&fragment, options.max_tokens)
                        .into_iter()
                        .map(|(s, _)| s)
                        .collect::<Vec<_>>()
                } else {
                    vec![fragment]
                }
            })
            .collect();
    }

    if let Some(heading) = section_heading {
        processed = prepend_heading_to_fragments(processed, heading, options);
    }

    processed
}
