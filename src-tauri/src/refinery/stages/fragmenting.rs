use crate::refinery::types::AtomFragment;
use crate::db::hash::compute_file_hash;
use crate::fragmenter::{fragment_paired, FragmentOptions};

pub async fn run_fragmenting_stage(content: &str, source_path: &str) -> Vec<AtomFragment> {
    let opts = FragmentOptions::default();
    let fragmented = fragment_paired(content.to_string(), &opts);
    
    let mut results = Vec::new();
    for (i, pair) in fragmented.pairs.into_iter().enumerate() {
        let hash = compute_file_hash(&pair.embedding);
        results.push(AtomFragment {
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
