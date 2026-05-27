use std::env;
use std::path::PathBuf;
use rusqlite::Connection;

fn get_db_path() -> PathBuf {
    if let Some(mut path) = dirs::data_dir() {
        path.push("scribo");
        path.push("scribo_core.db");
        if path.exists() {
            return path;
        }
    }
    let mut path = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    path.push("scribo_core.db");
    path
}

pub fn handle_cli(args: Vec<String>) {
    let db_path = get_db_path();
    let mut conn = Connection::open(&db_path).expect("Failed to open database");

    if let Err(e) = crate::db::schema::initialize_schema(&mut conn) {
        eprintln!("Warning: Failed to initialize schema: {}", e);
    }

    let command = &args[1];
    match command.as_str() {
        "status" => {
            println!("Database path: {}", db_path.display());
            println!("Ready to accept commands.");
        }
        "add" => {
            if args.len() < 4 {
                println!("Usage: scribo add <title> <content>");
                return;
            }
            let title = &args[2];
            let content = &args[3];
            let note = crate::domain::note::NewNote {
                title: title.clone(),
                content: content.clone(),
                ..Default::default()
            };
            let note_id = crate::db::repos::notes::insert(&conn, &note).unwrap();
            println!("Successfully added note: '{}' (ID: {})", title, note_id.0);
        }
        "import-dir" => {
            if args.len() < 3 {
                println!("Usage: scribo import-dir <path>");
                return;
            }
            let dir_path = PathBuf::from(&args[2]);
            if !dir_path.is_dir() {
                println!("Error: Path is not a directory.");
                return;
            }

            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap();

            rt.block_on(async {
                let embedder_config = crate::ai::types::EmbedderConfig {
                    provider: "local".to_string(),
                    model: Some("granite-embedding-97M-multilingual-r2-BF16".to_string()),
                    api_key: None,
                    base_url: None,
                };
                let embedder = crate::ai::embedding::Embedder::new(embedder_config);

                let mut md_files = Vec::new();
                for entry in walkdir::WalkDir::new(dir_path).into_iter().filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("md") {
                        md_files.push(path.to_path_buf());
                    }
                }

                println!("Found {} markdown files to import.", md_files.len());

                let mut imported = 0;
                for path in md_files {
                    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown");
                    let note_id = match crate::services::import::import_markdown_file(&conn, &path) {
                        Ok(id) => id,
                        Err(e) => {
                            eprintln!("Error importing file {:?}: {}", path, e);
                            continue;
                        }
                    };

                    let payload = crate::services::indexer::IndexingPayload {
                        note_id: note_id.0,
                        embedding_model: "granite-embedding-97M-multilingual-r2-BF16",
                        embedding_dim: 384,
                        indexing_version: "1",
                    };

                    if let Err(e) = crate::services::indexer::persist_indexed_file(&mut conn, payload) {
                        eprintln!("Error indexing file {}: {}", file_name, e);
                        continue;
                    }

                    let fragments = match crate::db::repos::fragments::list_by_note(&conn, note_id.0) {
                        Ok(frags) => frags,
                        Err(e) => {
                            eprintln!("Error listing fragments for {}: {}", file_name, e);
                            continue;
                        }
                    };

                    let mut fragment_embeddings = Vec::new();
                    for frag in &fragments {
                        match embedder.embed(&frag.text_clean).await {
                            Ok(emb) => {
                                fragment_embeddings.push((frag.fragment_index, emb));
                            }
                            Err(e) => {
                                eprintln!("Error embedding fragment for {}: {}", file_name, e);
                            }
                        }
                    }

                    let mut error_occurred = false;
                    for (index, emb) in fragment_embeddings {
                        let emb_bytes = bytemuck::cast_slice::<f32, u8>(&emb);
                        if let Err(e) = crate::db::repos::fragments::set_embedding(&conn, note_id.0, index, emb_bytes) {
                            eprintln!("Error saving embedding for {}: {}", file_name, e);
                            error_occurred = true;
                            break;
                        }
                    }

                    if !error_occurred {
                        println!("Imported: {} (fragments: {})", file_name, fragments.len());
                        imported += 1;
                    }
                }

                println!("Successfully imported {} markdown notes with embeddings.", imported);
            });
        }
        "fragment-file" => {
            if args.len() < 3 {
                println!("Usage: scribo fragment-file <file_path> [--embedding|--generation|--structural|--paired]");
                return;
            }
            let file_path = &args[2];
            let mode = args.get(3).map(|s| s.as_str()).unwrap_or("--paired");
            let content = std::fs::read_to_string(file_path).expect("Could not read file");
            let default_opts = crate::fragmenter::FragmentOptions::default();
            
            println!("File: {}", file_path);
            
            match mode {
                "--embedding" => {
                    let fragments = crate::fragmenter::fragment_for_embedding(&content, &default_opts);
                    println!("Total Fragments (Embedding): {}", fragments.len());
                    for (i, fragment) in fragments.iter().enumerate() {
                        println!("\n================ FRAGMENT {} ================", i);
                        println!("[Tokens: {}]", crate::fragmenter::stages::token::count_tokens(fragment));
                        println!("{}", fragment);
                    }
                }
                "--generation" => {
                    let fragments = crate::fragmenter::fragment_for_generation(&content, &default_opts);
                    println!("Total Fragments (Generation): {}", fragments.len());
                    for (i, fragment) in fragments.iter().enumerate() {
                        println!("\n================ FRAGMENT {} ================", i);
                        println!("[Tokens: {}]", crate::fragmenter::stages::token::count_tokens(fragment));
                        println!("{}", fragment);
                    }
                }
                "--structural" => {
                    let struct_opts = default_opts.for_mode(crate::fragmenter::FragmentMode::Structural);
                    let result = crate::fragmenter::fragment_paired(content, &struct_opts);
                    println!("Total Fragments (Structural): {}", result.pairs.len());
                    for (i, pair) in result.pairs.iter().enumerate() {
                        println!("\n================ FRAGMENT {} ================", i);
                        println!("[Tokens: {}]", crate::fragmenter::stages::token::count_tokens(&pair.embedding));
                        println!("{}", pair.embedding);
                    }
                }
                _ => { // --paired
                    let result = crate::fragmenter::fragment_paired(content, &default_opts);
                    println!("Total Fragments (Paired): {}", result.pairs.len());
                    for (i, pair) in result.pairs.iter().enumerate() {
                        println!("\n================ FRAGMENT {} ================", i);
                        println!("[Tokens: {}]", crate::fragmenter::stages::token::count_tokens(&pair.generation));
                        println!("[Embedding]:\n{}\n", pair.embedding);
                        println!("[Generation]:\n{}", pair.generation);
                    }
                }
            }
        }
        "list" => {
            let notes = crate::db::repos::notes::get_all(&conn).unwrap();
            println!("--- SCRIBO NOTES ---");
            for note in notes {
                println!("[ID: {}] {}", note.note_id.0, note.title);
            }
        }
        "search" => {
            if args.len() < 3 {
                println!("Usage: scribo search <query>");
                return;
            }
            let query = &args[2];
            let mut stmt = conn.prepare(
                "SELECT c.fragment_id, f.title, c.text_clean 
                 FROM fragments_fts 
                 JOIN fragments c ON c.fragment_id = fragments_fts.rowid
                 JOIN notes f ON f.note_id = c.note_id
                 WHERE fragments_fts MATCH ? LIMIT 5"
            ).unwrap();
            let rows = stmt.query_map([query], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
            }).unwrap();
            println!("Search results for '{}':", query);
            for row in rows {
                let (id, title, text) = row.unwrap();
                println!("-> {} (Fragment ID: {})\n   \"{}\"\n", title, id, text.trim());
            }
        }
        "add-draft" => {
            if args.len() < 3 {
                println!("Usage: scribo add-draft <content>");
                return;
            }
            let content = &args[2];
            let title = format!("Draft {}", chrono::Local::now().format("%Y-%m-%d %H:%M"));
            let note = crate::domain::note::NewNote {
                title,
                content: content.clone(),
                is_draft: true,
                ..Default::default()
            };
            let note_id = crate::db::repos::notes::insert(&conn, &note).unwrap();
            println!("Successfully created draft note with ID: {}", note_id.0);
        }
        "distribute" => {
            if args.len() < 3 {
                println!("Usage: scribo distribute <draft_id>");
                return;
            }
            let draft_id: i64 = args[2].parse().expect("Invalid draft ID");

            let models = crate::ai::models::scanner::scan_models();
            let llm_config = if let Some(llm_model) = models.iter().find(|m| matches!(m.kind, crate::ai::models::scanner::ModelKind::Llm)) {
                println!("Using local LLM model: {}", llm_model.id);
                crate::ai::LlmConfig {
                    backend: "local".to_string(),
                    model: llm_model.id.clone(),
                    api_key: None,
                    base_url: None,
                    system_prompt: None,
                    max_tokens: None,
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

            let manager = r2d2_sqlite::SqliteConnectionManager::file(&db_path);
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
                println!("Analyzing draft ID {}...", draft_id);
                let plan = match crate::services::distribute::analyze_draft_for_distribution(&state, draft_id, &llm_service).await {
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
                    println!("Recommendation: Action = \"{}\"", chunk.recommendation.action);
                    if let Some(target) = chunk.recommendation.target_note_id {
                        println!("  Target Note ID: {}", target);
                    }
                    if let Some(ref title) = chunk.recommendation.new_note_title {
                        println!("  New Note Title: \"{}\"", title);
                    }
                    if let Some(parent) = chunk.recommendation.parent_note_id {
                        println!("  Parent Note ID: {}", parent);
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
                        match crate::services::distribute::apply_distribution(&mut conn, plan) {
                            Ok(_) => println!("Plan successfully applied and original draft archived!"),
                            Err(e) => println!("Failed to apply plan: {}", e),
                        }
                    } else {
                        println!("Distribution cancelled.");
                    }
                }
            });
        }
        _ => {
            println!("Unknown command. Available commands: add, add-draft, distribute, list, search, import-dir, fragment-file");
        }
    }
}
