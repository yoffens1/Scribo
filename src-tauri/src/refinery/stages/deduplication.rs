use crate::refinery::types::{AtomFragment, DeduplicationResult, FragmentDecision};
use crate::DbState;

pub async fn run_deduplication_stage(fragments: Vec<AtomFragment>, state: &DbState) -> DeduplicationResult {
    use crate::refinery::constants::{MIN_CHUNK_LENGTH_FOR_MERGE, DEDUP_TOP_K, MERGE_SIMILARITY_THRESHOLD, NEAR_DUP_THRESHOLD};
    use crate::retrieval::pipeline::retrieve;
    use crate::retrieval::types::{RetrievalConfig, RetrieveOptions};
    
    let mut decisions = Vec::new();
    let mut remaining = Vec::new();

    let retrieval_config = RetrievalConfig {
        mode: "keyword".to_string(),
        embedding_weight: None,
        vault_lang: Some("en".to_string()),
        llm_config: None,
        pipeline: None,
        ai_rerank: None,
    };
    
    let retrieve_opts = RetrieveOptions {
        top_k: Some(DEDUP_TOP_K),
        filters: None,
    };

    for fragment in fragments {
        if fragment.embedding_text.len() < MIN_CHUNK_LENGTH_FOR_MERGE {
            decisions.push(FragmentDecision::Keep { fragment: fragment.clone(), reason: "too short to merge".to_string() });
            remaining.push(fragment.clone());
            continue;
        }

        let mut match_found = false;
        
        if let Ok(results) = retrieve(state, &fragment.embedding_text, None, &retrieval_config, &retrieve_opts).await {
            let filtered: Vec<_> = results.into_iter().filter(|r| r.fragment_ref.file_path != fragment.source_path).collect();
            if let Some(best) = filtered.first() {
                if best.score >= NEAR_DUP_THRESHOLD {
                    decisions.push(FragmentDecision::Reject {
                        fragment: fragment.clone(),
                        reason: format!("near-exact duplicate of {} (score: {:.3})", best.fragment_ref.file_path, best.score)
                    });
                    match_found = true;
                } else if best.score >= MERGE_SIMILARITY_THRESHOLD {
                    decisions.push(FragmentDecision::Merge {
                        target_path: best.fragment_ref.file_path.clone(),
                        source_fragment: fragment.clone(),
                        reason: format!("similar to {} (score: {:.3})", best.fragment_ref.file_path, best.score)
                    });
                    match_found = true;
                }
            }
        }

        if !match_found {
            decisions.push(FragmentDecision::Keep { fragment: fragment.clone(), reason: "no similar fragment found".to_string() });
            remaining.push(fragment);
        }
    }

    DeduplicationResult { decisions, remaining }
}
