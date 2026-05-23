use crate::refinery::types::AtomChunk;
use crate::db::hash::compute_file_hash;
use crate::chunker::{chunk_paired, ChunkOptions};

pub async fn run_chunking_stage(content: &str, source_path: &str) -> Vec<AtomChunk> {
    let opts = ChunkOptions::default();
    let chunked = chunk_paired(content.to_string(), &opts);
    
    let mut results = Vec::new();
    for (i, pair) in chunked.pairs.into_iter().enumerate() {
        let hash = compute_file_hash(&pair.embedding);
        results.push(AtomChunk {
            sources: vec![],
            hash,
            embedding_text: pair.embedding.clone(),
            generation_text: pair.generation,
            index: i,
            source_path: source_path.to_string(),
            is_table: false,
            question_heading: None,
            filename: None,
            aliases: Vec::new(),
            tags: Vec::new(),
        });
    }
    
    results
}
