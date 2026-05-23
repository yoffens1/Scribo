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
        // Stage 1: Chunking
        let raw_chunks = run_chunking_stage(content, source_path).await;
        
        // Stage 2: SubSplit
        let mut chunks = run_subsplit_stage(raw_chunks).await;

        // Stage 3: Consolidation
        chunks = run_consolidation_stage(chunks).await;

        // Stage 4: Deduplication
        let dedup = run_deduplication_stage(chunks, db_state).await;
        chunks = dedup.remaining;

        // Stage 5: Atomization (LLM)
        chunks = run_atomization_stage(chunks, Arc::clone(&self.llm)).await;

        // Stage 6: Enrichment
        chunks = run_enrichment_stage(chunks, Arc::clone(&self.llm)).await;

        // Stage 7: Taxonomy (LLM)
        let taxonomy = run_taxonomy_stage(&chunks, Arc::clone(&self.llm)).await;

        // Stage 8: Placement
        let placement = run_placement_stage(&chunks, &taxonomy, Arc::clone(&self.llm)).await;

        // Stage 9: Write
        run_write_stage(&chunks, &placement, db_state, Arc::clone(&self.llm)).await;

        Ok(placement)
    }
}

