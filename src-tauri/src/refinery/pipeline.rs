use std::sync::Arc;
use crate::ai::LlmService;
use crate::refinery::types::*;
use crate::refinery::stages::*;

pub struct RefineryPipeline {
    llm: Arc<LlmService>,
}

impl RefineryPipeline {
    pub fn new(llm: Arc<LlmService>) -> Self {
        Self { llm }
    }

    pub async fn refine(&self, source_path: &str, content: &str, db_state: &crate::DbState) -> Result<PlacementPlan, String> {
        // Stage 1: Fragmenting
        let raw_fragments = run_fragmenting_stage(content, source_path).await;
        
        // Stage 2: SubSplit
        let mut fragments = run_subsplit_stage(raw_fragments).await;

        // Stage 3: Consolidation
        fragments = run_consolidation_stage(fragments).await;

        // Stage 4: Deduplication
        let dedup = run_deduplication_stage(fragments, db_state).await;
        fragments = dedup.remaining;

        // Stage 5: Atomization (LLM)
        fragments = run_atomization_stage(fragments, Arc::clone(&self.llm)).await;

        // Stage 6: Enrichment
        fragments = run_enrichment_stage(fragments, Arc::clone(&self.llm)).await;

        // Stage 7: Taxonomy (LLM)
        let taxonomy = run_taxonomy_stage(&fragments, Arc::clone(&self.llm)).await;

        // Stage 8: Placement
        let placement = run_placement_stage(&fragments, &taxonomy, Arc::clone(&self.llm)).await;

        // Stage 9: Write
        run_write_stage(&fragments, &placement, db_state, Arc::clone(&self.llm)).await;

        Ok(placement)
    }
}

