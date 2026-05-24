use crate::refinery::types::AtomFragment;
use std::collections::HashSet;

pub async fn run_consolidation_stage(fragments: Vec<AtomFragment>) -> Vec<AtomFragment> {
    if fragments.len() <= 1 {
        return fragments;
    }

    let mut result = Vec::new();
    let mut merged = HashSet::new();
    let similarity_threshold = 0.95;

    for i in 0..fragments.len() {
        if merged.contains(&i) {
            continue;
        }

        let mut best = fragments[i].clone();
        for j in i + 1..fragments.len() {
            if merged.contains(&j) {
                continue;
            }

            let words_a: HashSet<&str> = best.embedding_text.split_whitespace().collect();
            let words_b: HashSet<&str> = fragments[j].embedding_text.split_whitespace().collect();
            let intersection = words_a.intersection(&words_b).count() as f64;
            let union = words_a.union(&words_b).count() as f64;
            let sim = if union == 0.0 { 0.0 } else { intersection / union };

            if sim >= similarity_threshold {
                if best.embedding_text.len() < fragments[j].embedding_text.len() {
                    best = fragments[j].clone();
                }
                merged.insert(j);
            }
        }
        result.push(best);
    }
    result
}
