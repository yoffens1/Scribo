//! # CLI Draft Distribution Handler
//!
//! Subcommand handler that runs the thematic draft distribution pipeline.
//! Recommends placement for draft sections, presents a preview to the user,
//! and applies changes to SQLite if approved.

use std::path::Path;
use rusqlite::Connection;

/// Runs the draft distribution pipeline in CLI mode.
/// Analyzes the draft note with the LLM, prints the distribution plan,
/// prompts the user for confirmation, and executes migrations, vectorization, and card generation.
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

    let llm_service = std::sync::Arc::new(crate::ai::LlmService::new(llm_config.clone(), None));

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async {
        println!("[Analyze] Starting note analysis...");
        let plan = match crate::services::distribute::analyze_draft_for_distribution(&state, note_id, &llm_service).await {
            Ok(p) => p,
            Err(e) => {
                println!("[Analyze] Analysis failed: {}", e);
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
                println!("[Apply] Applying distribution plan to database...");
                let affected_note_ids = match crate::services::distribute::apply_distribution(conn, plan) {
                    Ok(ids) => {
                        println!("[Apply] Distribution plan applied successfully! Original note archived. Affected note IDs: {:?}", ids);
                        ids
                    }
                    Err(e) => {
                        println!("[Apply] Failed to apply plan: {}", e);
                        return;
                    }
                };

                // Indexer stage
                println!("[Indexer] Updating indexing state and parsing sections...");
                for &id in &affected_note_ids {
                    let payload = crate::services::indexer::IndexingPayload {
                        note_id: id,
                        embedding_model: &llm_config.model,
                        embedding_dim: crate::constants::EMBEDDING_DIM as u32,
                        indexing_version: "1",
                    };
                    if let Err(e) = crate::services::indexer::persist_indexed_file(conn, payload) {
                        println!("[Indexer] Failed to persist index for note {}: {}", id, e);
                    } else {
                        println!("[Indexer] Indexed note {}.", id);
                    }
                }

                // Embedder stage
                println!("[Embedder] Computing fragment embeddings...");
                for &id in &affected_note_ids {
                    let fragments = match crate::db::repos::fragments::list_by_note(conn, id) {
                        Ok(frags) => frags,
                        Err(e) => {
                            println!("[Embedder] Failed to list fragments for note {}: {}", id, e);
                            continue;
                        }
                    };

                    let model_name = &llm_config.model;
                    let mut final_embeddings = vec![None; fragments.len()];
                    let mut cache_miss_indices = Vec::new();
                    let mut cache_miss_texts = Vec::new();

                    for (idx, frag) in fragments.iter().enumerate() {
                        let cache_hit: Option<Vec<u8>> = conn.query_row(
                            "SELECT embedding FROM embedding_cache WHERE clean_text_hash = ? AND embedding_model = ? AND embedding_model_version = '1'",
                            rusqlite::params![frag.clean_hash, model_name],
                            |r| r.get(0)
                        ).ok();

                        if let Some(bytes) = cache_hit {
                            final_embeddings[idx] = Some(bytes);
                        } else {
                            cache_miss_indices.push(idx);
                            cache_miss_texts.push(frag.text_clean.clone());
                        }
                    }

                    if !cache_miss_texts.is_empty() {
                        println!("[Embedder] Cache miss for {} fragment(s) in note {}. Requesting AI embeddings...", cache_miss_texts.len(), id);
                        if let Ok(embs) = llm_service.generate_embeddings(cache_miss_texts).await {
                            for (i, emb) in embs.into_iter().enumerate() {
                                let emb_bytes = bytemuck::cast_slice::<f32, u8>(&emb);
                                let frag_idx = cache_miss_indices[i];
                                let clean_hash = &fragments[frag_idx].clean_hash;
                                
                                // Insert into cache
                                let _ = conn.execute(
                                    "INSERT OR REPLACE INTO embedding_cache (clean_text_hash, embedding_model, embedding_model_version, embedding, created_at)
                                     VALUES (?, ?, '1', ?, strftime('%s','now'))",
                                    rusqlite::params![clean_hash, model_name, emb_bytes],
                                );
                                
                                final_embeddings[frag_idx] = Some(emb_bytes.to_vec());
                            }
                        } else {
                            println!("[Embedder] Error: Failed to generate embeddings from AI service.");
                        }
                    }

                    // Save fragment embeddings and compute mean-pooled section embeddings
                    for (idx, frag) in fragments.iter().enumerate() {
                        if let Some(ref emb_bytes) = final_embeddings[idx] {
                            let frag_idx = frag.fragment_index;
                            if let Err(e) = crate::db::repos::fragments::set_embedding(conn, id, frag_idx, emb_bytes) {
                                println!("[Embedder] Failed to set embedding for note {} fragment {}: {}", id, frag_idx, e);
                            }
                        }
                    }

                    // Compute section embeddings via mean pooling
                    if let Err(e) = crate::commands::distribute::compute_and_save_section_embeddings(conn, id) {
                        println!("[Embedder] Failed to compute section embeddings for note {}: {}", id, e);
                    } else {
                        println!("[Embedder] Section embeddings pooled and updated for note {}.", id);
                    }
                }

                // Refresh stale cards
                println!("[RefreshCards] Checking and regenerating flashcards for affected notes...");
                match crate::services::distribute::refresh_stale_cards_for_notes(&state, &affected_note_ids, &llm_service).await {
                    Ok(_) => println!("[RefreshCards] Card refresh completed successfully."),
                    Err(e) => println!("[RefreshCards] Error during card refresh: {}", e),
                }

                println!("[Pipeline] Distribution pipeline finished successfully!");
            } else {
                println!("Distribution cancelled.");
            }
        }
    });
}
