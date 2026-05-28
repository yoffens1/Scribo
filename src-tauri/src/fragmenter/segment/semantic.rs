use crate::fragmenter::config::FragmentConfig;
use crate::fragmenter::segment::{Atom, AtomKind};
use crate::fragmenter::token::count_tokens;
use crate::utils::cosine_similarity;
use crate::ai::LlmService;

pub async fn segment(
    content: &str,
    max_chars: usize,
    threshold: f32,
    llm_service: &LlmService,
) -> Vec<Atom> {
    // 1. Get initial block-level atoms
    let raw_atoms = super::block::segment(content, &FragmentConfig::default());
    if raw_atoms.is_empty() {
        return Vec::new();
    }

    // 2. Generate embeddings
    let texts: Vec<String> = raw_atoms.iter().map(|a| a.text.clone()).collect();
    let embeddings = match llm_service.generate_embeddings(texts.clone()).await {
        Ok(embs) => embs,
        Err(_) => vec![vec![0.0f32; 1536]; texts.len()],
    };

    // 3. Merge atoms based on cosine similarity and length threshold
    let mut merged_atoms = Vec::new();
    let mut current_group: Vec<Atom> = vec![raw_atoms[0].clone()];

    for i in 1..raw_atoms.len() {
        let sim = if i < embeddings.len() && i - 1 < embeddings.len() {
            cosine_similarity(&embeddings[i - 1], &embeddings[i])
        } else {
            0.0
        };

        let total_len: usize = current_group.iter().map(|a| a.text.len()).sum();
        let next_len = raw_atoms[i].text.len();
        let next_is_heading = matches!(raw_atoms[i].kind, AtomKind::Heading { .. });

        if sim < threshold || (total_len + next_len > max_chars) || next_is_heading {
            // Flush current group as one Atom
            let merged_atom = merge_group(&current_group);
            merged_atoms.push(merged_atom);
            current_group.clear();
        }

        current_group.push(raw_atoms[i].clone());
    }

    if !current_group.is_empty() {
        let merged_atom = merge_group(&current_group);
        merged_atoms.push(merged_atom);
    }

    merged_atoms
}

fn merge_group(group: &[Atom]) -> Atom {
    let text = group.iter().map(|a| a.text.as_str()).collect::<Vec<_>>().join("\n\n");
    let heading_title = group.iter().find_map(|a| a.heading_title.clone());
    let is_top_level = group.iter().any(|a| a.is_top_level);
    
    let start = group.first().and_then(|a| a.range.clone()).map(|r| r.start).unwrap_or(0);
    let end = group.last().and_then(|a| a.range.clone()).map(|r| r.end).unwrap_or(text.len());
    
    let tokens = count_tokens(&text);
    
    Atom {
        text: text.clone(),
        kind: AtomKind::Paragraph,
        range: Some(start..end),
        heading_path: Vec::new(),
        heading_title,
        is_top_level,
        token_count: tokens,
        char_count: text.len(),
    }
}
