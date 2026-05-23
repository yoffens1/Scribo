use crate::chunker::types::ChunkOptions;
use super::clean::clean_chunk;

pub fn prepend_heading_to_chunks(chunks: Vec<String>,
    section_heading: &str,
    options: &ChunkOptions,
) -> Vec<String> {
    let clean_heading = clean_chunk(section_heading, options).trim().to_string();
    if clean_heading.is_empty() {
        return chunks;
    }

    chunks
        .into_iter()
        .filter(|chunk| chunk != &clean_heading)
        .map(|chunk| {
            let first_line = chunk.trim_start().lines().next().unwrap_or("").trim();
            if first_line == clean_heading {
                chunk
            } else {
                format!("{}\n{}", clean_heading, chunk)
            }
        })
        .collect()
}
