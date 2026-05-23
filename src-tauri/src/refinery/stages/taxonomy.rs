use crate::refinery::types::{AtomChunk, ProposedTaxonomy};
use crate::ai::LlmService;
use crate::ai::prompts::{build_taxonomy_prompt, ChunkForTaxonomy};
use std::sync::Arc;

pub async fn run_taxonomy_stage(chunks: &[AtomChunk], llm: Arc<LlmService>) -> ProposedTaxonomy {
    let taxonomy_chunks: Vec<ChunkForTaxonomy> = chunks.iter().map(|c| ChunkForTaxonomy {
        hash: &c.hash,
        text: &c.generation_text,
        source_path: &c.source_path,
    }).collect();

    let messages = build_taxonomy_prompt(&taxonomy_chunks, 3);
    
    if let Ok(response) = llm.generate_messages(messages).await {
        if let Ok(parsed) = serde_json::from_str::<ProposedTaxonomy>(&response.text) {
            return parsed;
        }
    }
    
    ProposedTaxonomy { roots: Vec::new(), rationale: String::new() }
}
