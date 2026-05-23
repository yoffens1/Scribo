use crate::refinery::types::{AtomChunk, ProposedTaxonomy, PlacementPlan};
use crate::ai::LlmService;
use crate::ai::prompts::{build_placement_prompt, ChunkForTaxonomy};
use std::sync::Arc;

pub async fn run_placement_stage(chunks: &[AtomChunk], taxonomy: &ProposedTaxonomy, llm: Arc<LlmService>) -> PlacementPlan {
    let taxonomy_chunks: Vec<ChunkForTaxonomy> = chunks.iter().map(|c| ChunkForTaxonomy {
        hash: &c.hash,
        text: &c.generation_text,
        source_path: &c.source_path,
    }).collect();

    let proposed_tree_str = serde_json::to_string_pretty(&taxonomy.roots).unwrap_or_default();
    let existing_tree = "";

    let messages = build_placement_prompt(
        &proposed_tree_str,
        existing_tree,
        &taxonomy_chunks
    );

    if let Ok(response) = llm.generate_messages(messages).await {
        if let Ok(parsed) = serde_json::from_str::<PlacementPlan>(&response.text) {
            return parsed;
        }
    }
    
    PlacementPlan { decisions: Vec::new(), folders_to_create: Vec::new(), rationale: String::new() }
}
