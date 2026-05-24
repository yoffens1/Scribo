use crate::refinery::types::{AtomFragment, ProposedTaxonomy};
use crate::ai::LlmService;
use crate::ai::prompts::{build_taxonomy_prompt, FragmentForTaxonomy};
use std::sync::Arc;

pub async fn run_taxonomy_stage(fragments: &[AtomFragment], llm: Arc<LlmService>) -> ProposedTaxonomy {
    let taxonomy_fragments: Vec<FragmentForTaxonomy> = fragments.iter().map(|c| FragmentForTaxonomy {
        hash: &c.hash,
        text: &c.generation_text,
        source_path: &c.source_path,
    }).collect();

    let messages = build_taxonomy_prompt(&taxonomy_fragments, 3);
    
    if let Ok(response) = llm.generate_messages(messages).await {
        if let Ok(parsed) = serde_json::from_str::<ProposedTaxonomy>(&response.text) {
            return parsed;
        }
    }
    
    ProposedTaxonomy { roots: Vec::new(), rationale: String::new() }
}
