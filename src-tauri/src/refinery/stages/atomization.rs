use crate::refinery::types::AtomFragment;
use crate::ai::LlmService;
use crate::ai::prompts::build_atomize_prompt;
use std::sync::Arc;
use futures::stream::{self, StreamExt};

pub async fn run_atomization_stage(fragments: Vec<AtomFragment>, llm: Arc<LlmService>) -> Vec<AtomFragment> {
    let concurrency_limit = 5;
    
    let processed = stream::iter(fragments)
        .map(|mut fragment| {
            let llm = Arc::clone(&llm);
            async move {
                if fragment.generation_text.len() < 30 {
                    return fragment;
                }
                
                let messages = build_atomize_prompt(&fragment.generation_text, &fragment.source_path);
                if let Ok(response) = llm.generate_messages(messages).await {
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&response.text) {
                        if let Some(heading) = parsed.get("questionHeading").and_then(|v| v.as_str()) {
                            let mut h = heading.trim().to_string();
                            if !h.starts_with("## ") {
                                h = format!("## {}", h.trim_start_matches('#').trim_start());
                            }
                            fragment.question_heading = Some(h);
                        }
                        if let Some(filename) = parsed.get("filename").and_then(|v| v.as_str()) {
                            let mut f = filename.trim().to_string();
                            f = f.replace(&['<', '>', ':', '"', '/', '\\', '|', '?', '*'][..], "");
                            if !f.ends_with(".md") {
                                f.push_str(".md");
                            }
                            fragment.filename = Some(f);
                        }
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
