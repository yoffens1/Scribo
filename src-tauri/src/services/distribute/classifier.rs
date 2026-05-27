use crate::domain::distribute::{ChunkDistributionPlan, DistributeAction};

pub trait Classifier: Send + Sync {
    fn classify(&self, chunks: &mut [ChunkDistributionPlan]);
}

pub struct HeuristicClassifier;

impl HeuristicClassifier {
    pub fn new() -> Self {
        Self
    }
}

impl Classifier for HeuristicClassifier {
    fn classify(&self, chunks: &mut [ChunkDistributionPlan]) {
        apply_heuristic_linking(chunks);
    }
}

pub fn apply_heuristic_linking(chunks: &mut [ChunkDistributionPlan]) {
    let mut new_notes: Vec<(usize, String)> = Vec::new();
    for chunk in chunks.iter() {
        if let DistributeAction::CreateChild { new_note_title, .. } = &chunk.recommendation.action {
            let title = if new_note_title.is_empty() {
                chunk.suggested_title.clone()
            } else {
                new_note_title.clone()
            };
            new_notes.push((chunk.chunk_index, title));
        }
    }

    for i in 0..new_notes.len() {
        let (idx_a, title_a) = &new_notes[i];
        let normalized_a = title_a.to_lowercase();
        let words_a: std::collections::HashSet<&str> = normalized_a.split_whitespace().collect();

        for j in 0..new_notes.len() {
            if i == j { continue; }
            let (idx_b, title_b) = &new_notes[j];
            let normalized_b = title_b.to_lowercase();
            let words_b: std::collections::HashSet<&str> = normalized_b.split_whitespace().collect();

            let is_match = if words_b.iter().all(|w| words_a.contains(w)) {
                true
            } else {
                words_b.iter().any(|wb| {
                    wb.len() >= 4 && words_a.iter().any(|wa| wa.starts_with(wb) || wb.starts_with(wa))
                })
            };

            if is_match && title_a.len() > title_b.len() {
                if let DistributeAction::CreateChild { new_note_title, .. } = &chunks[*idx_a].recommendation.action {
                    chunks[*idx_a].recommendation.action = DistributeAction::CreateChild {
                        parent_note_id: Some(crate::domain::NoteId(-(*idx_b as i64 + 1))),
                        new_note_title: new_note_title.clone(),
                    };
                }
                break;
            }
        }
    }
}
