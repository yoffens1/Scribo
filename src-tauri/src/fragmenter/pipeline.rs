use rayon::prelude::*;
use crate::fragmenter::stages::extract;
use crate::fragmenter::types::{FragmentOptions, FragmenterPair, FragmenterResult, FragmentMode};

use super::stages::sections::fragment_by_heading_sections;
use super::stages::sections::process_section;
use super::stages::clean::clean_fragment;

pub fn run_pipeline(
    content: &str,
    options: &FragmentOptions,
) -> (Vec<String>, Option<serde_json::Map<String, serde_json::Value>>) {
    let (metadata, remaining_content) = extract::extract_yaml_frontmatter(content);

    let fragments = if options.fragment_by_headings {
        fragment_by_heading_sections(&remaining_content, options)
    } else {
        process_section(&remaining_content, options, None)
    };

    (fragments, metadata)
}

pub fn fragment_paired(content: String, options: &FragmentOptions) -> FragmenterResult {
    let (struct_fragments, metadata) = run_pipeline(&content, &options.for_mode(FragmentMode::Structural));

    let embed_opts = options.for_mode(FragmentMode::Embedding);
    let gen_opts = options.for_mode(FragmentMode::Generation);

    let pairs: Vec<FragmenterPair> = struct_fragments
        .par_iter()
        .map(|raw| FragmenterPair {
            embedding: clean_fragment(raw, &embed_opts),
            generation: clean_fragment(raw, &gen_opts),
        })
        .collect();

    FragmenterResult { pairs, metadata }
}

pub fn fragment_for_embedding(content: &str, options: &FragmentOptions) -> Vec<String> {
    let (fragments, _) = run_pipeline(content, &options.for_mode(FragmentMode::Embedding));
    fragments
}

pub fn fragment_for_generation(content: &str, options: &FragmentOptions) -> Vec<String> {
    let (fragments, _) = run_pipeline(content, &options.for_mode(FragmentMode::Generation));
    fragments
}
