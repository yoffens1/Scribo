use crate::refinery::types::AtomFragment;
use crate::ai::LlmService;
use std::sync::Arc;
use futures::stream::{self, StreamExt};

pub async fn run_enrichment_stage(fragments: Vec<AtomFragment>, llm: Arc<LlmService>) -> Vec<AtomFragment> {
    let concurrency_limit = 4;
    
    let processed = stream::iter(fragments)
        .map(|mut fragment| {
            let llm = Arc::clone(&llm);
            async move {
                if fragment.generation_text.len() < 50 {
                    return fragment;
                }
                
                let heading = fragment.question_heading.clone().unwrap_or_default();
                
                let alias_msgs = vec![
                    crate::ai::types::Message {
                        role: "system".to_string(),
                        content: "Generate up to 8 alternative aliases (synonyms) for the main concept in this fragment. Return ONLY a JSON array of strings: [\"alias1\", \"alias2\"].".to_string(),
                    },
                    crate::ai::types::Message {
                        role: "user".to_string(),
                        content: format!("Heading: {}\n\nText:\n{}", heading, fragment.generation_text),
                    }
                ];
                if let Ok(resp) = llm.generate_messages(alias_msgs).await {
                    if let Ok(parsed) = serde_json::from_str::<Vec<String>>(&resp.text) {
                        fragment.aliases = parsed.into_iter().take(8).collect();
                    }
                }
                
                let tags_msgs = vec![
                    crate::ai::types::Message {
                        role: "system".to_string(),
                        content: "Generate up to 6 relevant tags for this fragment. Use lowercase, single words or hyphenated words (e.g., 'machine-learning'). Return ONLY a JSON array of strings: [\"tag1\", \"tag2\"].".to_string(),
                    },
                    crate::ai::types::Message {
                        role: "user".to_string(),
                        content: format!("Heading: {}\n\nText:\n{}", heading, fragment.generation_text),
                    }
                ];
                if let Ok(resp) = llm.generate_messages(tags_msgs).await {
                    if let Ok(parsed) = serde_json::from_str::<Vec<String>>(&resp.text) {
                        fragment.tags = parsed.into_iter().take(6).collect();
                    }
                }
                
                fragment
            }
        })
        .buffer_unordered(concurrency_limit)
        .collect::<Vec<_>>()
        .await;

    processed
}
