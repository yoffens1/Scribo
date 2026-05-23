pub mod sections;
pub mod assemble;
pub mod tables;
pub mod sub_headings;
pub mod clean;
pub mod headings;

use rayon::prelude::*;
use crate::chunker::extract;
use crate::chunker::types::{ChunkOptions, ChunkerPair, ChunkerResult, ChunkMode};

use sections::chunk_by_heading_sections;
use sections::process_section;
use clean::clean_chunk;

pub fn run_pipeline(
    content: &str,
    options: &ChunkOptions,
) -> (Vec<String>, Option<serde_json::Map<String, serde_json::Value>>) {
    let (metadata, remaining_content) = extract::extract_yaml_frontmatter(content);

    let chunks = if options.chunk_by_headings {
        chunk_by_heading_sections(&remaining_content, options)
    } else {
        process_section(&remaining_content, options, None)
    };

    (chunks, metadata)
}

pub fn chunk_paired(content: String, options: &ChunkOptions) -> ChunkerResult {
    let (struct_chunks, metadata) = run_pipeline(&content, &options.for_mode(ChunkMode::Structural));

    let embed_opts = options.for_mode(ChunkMode::Embedding);
    let gen_opts = options.for_mode(ChunkMode::Generation);

    let pairs: Vec<ChunkerPair> = struct_chunks
        .par_iter()
        .map(|raw| ChunkerPair {
            embedding: clean_chunk(raw, &embed_opts),
            generation: clean_chunk(raw, &gen_opts),
        })
        .collect();

    ChunkerResult { pairs, metadata }
}

pub fn chunk_for_embedding(content: &str, options: &ChunkOptions) -> Vec<String> {
    let (chunks, _) = run_pipeline(content, &options.for_mode(ChunkMode::Embedding));
    chunks
}

pub fn chunk_for_generation(content: &str, options: &ChunkOptions) -> Vec<String> {
    let (chunks, _) = run_pipeline(content, &options.for_mode(ChunkMode::Generation));
    chunks
}
