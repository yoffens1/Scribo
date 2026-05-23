use crate::refinery::types::AtomChunk;
use crate::ai::LlmService;
use std::sync::Arc;
use futures::stream::{self, StreamExt};

pub async fn run_enrichment_stage(chunks: Vec<AtomChunk>, llm: Arc<LlmService>) -> Vec<AtomChunk> {
    let concurrency_limit = 4;
    
    let processed = stream::iter(chunks)
        .map(|mut chunk| {
            let llm = Arc::clone(&llm);
            async move {
                if chunk.generation_text.len() < 50 {
                    return chunk;
                }
                
                let heading = chunk.question_heading.clone().unwrap_or_default();
                
                let alias_msgs = vec![
                    crate::ai::types::Message {
                        role: "system".to_string(),
                        content: "Generate up to 8 alternative aliases (synonyms) for the main concept in this chunk. Return ONLY a JSON array of strings: [\"alias1\", \"alias2\"].".to_string(),
                    },
                    crate::ai::types::Message {
                        role: "user".to_string(),
                        content: format!("Heading: {}\n\nText:\n{}", heading, chunk.generation_text),
                    }
                ];
                if let Ok(resp) = llm.generate_messages(alias_msgs).await {
                    if let Ok(parsed) = serde_json::from_str::<Vec<String>>(&resp.text) {
                        chunk.aliases = parsed.into_iter().take(8).collect();
                    }
                }
                
                let tags_msgs = vec![
                    crate::ai::types::Message {
                        role: "system".to_string(),
                        content: "Generate up to 6 relevant tags for this chunk. Use lowercase, single words or hyphenated words (e.g., 'machine-learning'). Return ONLY a JSON array of strings: [\"tag1\", \"tag2\"].".to_string(),
                    },
                    crate::ai::types::Message {
                        role: "user".to_string(),
                        content: format!("Heading: {}\n\nText:\n{}", heading, chunk.generation_text),
                    }
                ];
                if let Ok(resp) = llm.generate_messages(tags_msgs).await {
                    if let Ok(parsed) = serde_json::from_str::<Vec<String>>(&resp.text) {
                        chunk.tags = parsed.into_iter().take(6).collect();
                    }
                }
                
                chunk
            }
        })
        .buffer_unordered(concurrency_limit)
        .collect::<Vec<_>>()
        .await;

    processed
}
