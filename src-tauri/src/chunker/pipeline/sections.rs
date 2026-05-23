use std::sync::LazyLock;
use rayon::prelude::*;
use crate::chunker::extract;
use crate::chunker::types::ChunkOptions;
use crate::chunker::table;
use crate::chunker::token;
use super::assemble::{glue_subheadings_to_content, assemble_raw_chunks};
use super::tables::{restore_tables, linearize_table_chunks};
use super::sub_headings::split_chunks_by_sub_headings;
use super::clean::clean_chunk;
use super::headings::prepend_heading_to_chunks;

static RE_PARA: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"\n\s*\n").unwrap());
static RE_HEADING_LEVELS: LazyLock<[regex::Regex; 6]> = LazyLock::new(|| [
    regex::Regex::new(r"^#{1,6}\s").unwrap(),
    regex::Regex::new(r"^#{2,6}\s").unwrap(),
    regex::Regex::new(r"^#{3,6}\s").unwrap(),
    regex::Regex::new(r"^#{4,6}\s").unwrap(),
    regex::Regex::new(r"^#{5,6}\s").unwrap(),
    regex::Regex::new(r"^#{6,6}\s").unwrap(),
]);

pub fn chunk_by_heading_sections(content: &str, options: &ChunkOptions) -> Vec<String> {
    let sections = extract::split_by_headings(content, options.heading_level);
    sections
        .par_iter()
        .flat_map(|section| {
            let section_heading = extract_section_heading(section, options);
            let target_heading = if options.include_heading_in_chunks {
                section_heading
            } else {
                None
            };
            process_section(section, options, target_heading.as_deref())
        })
        .collect()
}

pub fn extract_section_heading(section: &str, options: &ChunkOptions) -> Option<String> {
    let first_line = section.trim_start().lines().next()?.trim();
    let idx = options.heading_level.saturating_sub(1).min(5);
    let re = &RE_HEADING_LEVELS[idx];
    if re.is_match(first_line) {
        Some(first_line.to_string())
    } else {
        None
    }
}

pub fn process_section(text: &str, options: &ChunkOptions, section_heading: Option<&str>) -> Vec<String> {
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

    let raw_chunks = assemble_raw_chunks(paragraphs_cow, options);
    let mut merged_chunks = restore_tables(raw_chunks, &tables, options);

    if options.separate_sub_headings {
        merged_chunks = split_chunks_by_sub_headings(merged_chunks, options.heading_level);
    }

    merged_chunks = linearize_table_chunks(merged_chunks, options);

    let mut processed: Vec<String> = merged_chunks
        .iter()
        .map(|chunk| clean_chunk(chunk, options))
        .filter(|c| !c.is_empty())
        .collect();

    if options.max_tokens > 0 {
        processed = processed
            .into_iter()
            .flat_map(|chunk| {
                if token::count_tokens(&chunk) > options.max_tokens {
                    token::split_oversized_paragraph(&chunk, options.max_tokens)
                        .into_iter()
                        .map(|(s, _)| s)
                        .collect::<Vec<_>>()
                } else {
                    vec![chunk]
                }
            })
            .collect();
    }

    if let Some(heading) = section_heading {
        processed = prepend_heading_to_chunks(processed, heading, options);
    }

    processed
}
