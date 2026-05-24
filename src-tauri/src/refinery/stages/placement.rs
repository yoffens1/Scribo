use crate::refinery::types::{AtomFragment, ProposedTaxonomy, PlacementPlan};
use crate::ai::LlmService;
use crate::ai::prompts::{build_placement_prompt, FragmentForTaxonomy};
use std::sync::Arc;

pub async fn run_placement_stage(fragments: &[AtomFragment], taxonomy: &ProposedTaxonomy, llm: Arc<LlmService>) -> PlacementPlan {
    let taxonomy_fragments: Vec<FragmentForTaxonomy> = fragments.iter().map(|c| FragmentForTaxonomy {
        hash: &c.hash,
        text: &c.generation_text,
        source_path: &c.source_path,
    }).collect();

    let proposed_tree_str = serde_json::to_string_pretty(&taxonomy.roots).unwrap_or_default();
    let existing_tree = "";

    let messages = build_placement_prompt(
        &proposed_tree_str,
        existing_tree,
        &taxonomy_fragments
    );

    if let Ok(response) = llm.generate_messages(messages).await {
        if let Ok(parsed) = serde_json::from_str::<PlacementPlan>(&response.text) {
            return parsed;
        }
    }
    
    PlacementPlan { decisions: Vec::new(), folders_to_create: Vec::new(), rationale: String::new() }
}
