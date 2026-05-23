use crate::refinery::types::AtomChunk;
use std::collections::HashSet;

pub async fn run_consolidation_stage(chunks: Vec<AtomChunk>) -> Vec<AtomChunk> {
    if chunks.len() <= 1 {
        return chunks;
    }

    let mut result = Vec::new();
    let mut merged = HashSet::new();
    let similarity_threshold = 0.95;

    for i in 0..chunks.len() {
        if merged.contains(&i) {
            continue;
        }

        let mut best = chunks[i].clone();
        for j in i + 1..chunks.len() {
            if merged.contains(&j) {
                continue;
            }

            let words_a: HashSet<&str> = best.embedding_text.split_whitespace().collect();
            let words_b: HashSet<&str> = chunks[j].embedding_text.split_whitespace().collect();
            let intersection = words_a.intersection(&words_b).count() as f64;
            let union = words_a.union(&words_b).count() as f64;
            let sim = if union == 0.0 { 0.0 } else { intersection / union };

            if sim >= similarity_threshold {
                if best.embedding_text.len() < chunks[j].embedding_text.len() {
                    best = chunks[j].clone();
                }
                merged.insert(j);
            }
        }
        result.push(best);
    }
    result
}
