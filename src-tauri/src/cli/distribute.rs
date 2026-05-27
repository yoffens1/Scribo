use std::path::Path;
use rusqlite::Connection;

pub fn handle_distribute(conn: &mut Connection, db_path: &Path, note_id: i64) {
    let models = crate::ai::models::scanner::scan_models();
    let llm_config = if let Some(llm_model) = models.iter().find(|m| matches!(m.kind, crate::ai::models::scanner::ModelKind::Llm)) {
        println!("Using local LLM model: {}", llm_model.id);
        crate::ai::LlmConfig {
            backend: "local".to_string(),
            model: llm_model.id.clone(),
            api_key: None,
            base_url: None,
            system_prompt: None,
            max_tokens: Some(2048),
            temperature: None,
            response_format: Some("json".to_string()),
        }
    } else if let Ok(or_key) = std::env::var("OPENROUTER_API_KEY") {
        println!("No local LLM model found. Using OpenRouter (google/gemini-2.5-flash) with OPENROUTER_API_KEY.");
        crate::ai::LlmConfig {
            backend: "openai".to_string(),
            model: "google/gemini-2.5-flash".to_string(),
            api_key: Some(or_key),
            base_url: Some("https://openrouter.ai/api/v1".to_string()),
            system_prompt: None,
            max_tokens: None,
            temperature: None,
            response_format: Some("json".to_string()),
        }
    } else if let Ok(oa_key) = std::env::var("OPENAI_API_KEY") {
        println!("No local LLM model found. Using OpenAI (gpt-4o-mini) with OPENAI_API_KEY.");
        crate::ai::LlmConfig {
            backend: "openai".to_string(),
            model: "gpt-4o-mini".to_string(),
            api_key: Some(oa_key),
            base_url: None,
            system_prompt: None,
            max_tokens: None,
            temperature: None,
            response_format: Some("json".to_string()),
        }
    } else if let Ok(gem_key) = std::env::var("GEMINI_API_KEY") {
        println!("No local LLM model found. Using Gemini (gemini-1.5-flash) with GEMINI_API_KEY.");
        crate::ai::LlmConfig {
            backend: "gemini".to_string(),
            model: "gemini-1.5-flash".to_string(),
            api_key: Some(gem_key),
            base_url: None,
            system_prompt: None,
            max_tokens: None,
            temperature: None,
            response_format: Some("json".to_string()),
        }
    } else {
        println!("Error: No local LLM models (.gguf) found in the models directory, and no API keys (OPENROUTER_API_KEY, OPENAI_API_KEY, GEMINI_API_KEY) found in the environment.");
        return;
    };

    let manager = r2d2_sqlite::SqliteConnectionManager::file(db_path);
    let pool = r2d2::Pool::builder()
        .max_size(2)
        .build(manager)
        .expect("Failed to build pool");
    let state = crate::DbState::new();
    *state.pool.write() = Some(pool);

    let llm_service = std::sync::Arc::new(crate::ai::LlmService::new(llm_config, None));

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async {
        println!("Analyzing note ID {}...", note_id);
        let plan = match crate::services::distribute::analyze_draft_for_distribution(&state, note_id, &llm_service).await {
            Ok(p) => p,
            Err(e) => {
                println!("Analysis failed: {}", e);
                return;
            }
        };

        println!("\n=== DISTRIBUTION PREVIEW ===");
        for (i, chunk) in plan.chunks.iter().enumerate() {
            println!("\nChunk {}: Suggested Title: \"{}\"", i + 1, chunk.suggested_title);
            println!("Text:\n  {}", chunk.text.replace("\n", "\n  "));
            println!("Recommendation: Action = \"{:?}\"", chunk.recommendation.action);
            match &chunk.recommendation.action {
                crate::domain::distribute::DistributeAction::Append { target_note_id, target_section_id } => {
                    println!("  Target Note ID: {}", target_note_id.0);
                    if let Some(sec_id) = target_section_id {
                        println!("  Target Section ID: {}", sec_id.0);
                    }
                }
                crate::domain::distribute::DistributeAction::CreateChild { parent_note_id, new_note_title } => {
                    println!("  New Note Title: \"{}\"", new_note_title);
                    if let Some(parent) = parent_note_id {
                        println!("  Parent Note ID: {}", parent.0);
                    }
                }
                crate::domain::distribute::DistributeAction::MergeWithChunk { chunk_index } => {
                    println!("  Merge with Chunk Index: {}", chunk_index);
                }
                crate::domain::distribute::DistributeAction::Skip => {
                    // Reason is already printed under general recommendation details
                }
            }
            println!("  Reason: {}", chunk.recommendation.reason);
        }
        println!("============================\n");

        print!("Apply this distribution plan? [y/N]: ");
        use std::io::Write;
        let _ = std::io::stdout().flush();
        let mut input = String::new();
        if std::io::stdin().read_line(&mut input).is_ok() {
            let trimmed = input.trim().to_lowercase();
            if trimmed == "y" || trimmed == "yes" {
                match crate::services::distribute::apply_distribution(conn, plan) {
                    Ok(_) => println!("Plan successfully applied and original note archived!"),
                    Err(e) => println!("Failed to apply plan: {}", e),
                }
            } else {
                println!("Distribution cancelled.");
            }
        }
    });
}
