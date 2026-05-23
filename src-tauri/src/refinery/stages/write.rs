use crate::refinery::types::{AtomChunk, PlacementPlan};
use crate::DbState;
use crate::ai::LlmService;
use std::sync::Arc;

pub async fn run_write_stage(chunks: &[AtomChunk], placement: &PlacementPlan, state: &DbState, llm: Arc<LlmService>) {
    use crate::refinery::writers::{FileWriter, FileWriterContext, TransactionalWriter};
    
    let ctx = FileWriterContext {
        llm: Some(llm),
        output_root: "/home/yoffens/obsidian2026/.obsidian/plugins/LLM-Assist/src/test/test-db/".to_string(),
        overwrite_on_merge: true,
        merge_tags: true,
        delete_from_db_on_gc: true,
    };
    
    let writer = FileWriter::new(ctx);
    let mut tx_writer = TransactionalWriter::new(writer);
    
    let mut operations = Vec::new();
    
    for decision in &placement.decisions {
        let chunk_opt = chunks.iter().find(|c| c.hash == decision.chunk_hash);
        if let Some(chunk) = chunk_opt {
            let mut content = String::from("---\n");
            if !chunk.aliases.is_empty() {
                let aliases_str = chunk.aliases.iter().map(|a| format!("\"{}\"", a)).collect::<Vec<_>>().join(", ");
                content.push_str(&format!("aliases: [{}]\n", aliases_str));
            }
            if !chunk.tags.is_empty() {
                let tags_str = chunk.tags.iter().map(|t| format!("\"{}\"", t)).collect::<Vec<_>>().join(", ");
                content.push_str(&format!("tags: [{}]\n", tags_str));
            }
            content.push_str("---\n\n");

            if let Some(heading) = &chunk.question_heading {
                let text = chunk.generation_text.replacen(&format!("{} ", heading), "", 1);
                content.push_str(&format!("{}\n{}", heading, text.trim_start()));
            } else {
                content.push_str(&chunk.generation_text);
            }

            let file_path = std::path::Path::new("/home/yoffens/obsidian2026/.obsidian/plugins/LLM-Assist/src/test/test-db/").join(&decision.output_path);
            let path_str = file_path.to_string_lossy().into_owned();

            match decision.action.as_str() {
                "create" => {
                    operations.push(crate::refinery::types::WriteOperation::CreateFile { path: path_str, content });
                }
                "merge" | "rename" => {
                    operations.push(crate::refinery::types::WriteOperation::MergeChunk { 
                        source_file: chunk.source_path.clone(), 
                        target_file: path_str, 
                        chunk_text: content 
                    });
                }
                _ => {}
            }
        }
    }
    
    let _ = tx_writer.execute_batch(operations, None, Some(state)).await;
}
