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
    // Для удобства разработки создаем базу прямо в текущей директории репозитория
    let mut path = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    path.push("scribo_core.db");
    path
}

pub fn handle_cli(args: Vec<String>) {
    let db_path = get_db_path();
    let mut conn = Connection::open(&db_path).expect("Failed to open database");

    // Ensure the database is initialized before CLI actions
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
            conn.execute(
                "INSERT INTO files (file_path, file_name, status, updated_at) VALUES (?1, ?1, 'indexed', ?2)",
                (title, 12345_i64),
            ).unwrap();
            let file_id = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO chunks (file_id, chunk_index, chunk_text, embedding) VALUES (?, 0, ?, X'00')",
                (file_id, content),
            ).unwrap();
            println!("Successfully added note: '{}' (ID: {})", title, file_id);
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
            let mut imported = 0;
            
            let tx = conn.transaction().unwrap();

            for entry in walkdir::WalkDir::new(dir_path).into_iter().filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("md") {
                    let title = path.file_name().unwrap().to_str().unwrap().to_string();
                    let content = std::fs::read_to_string(path).unwrap_or_default();
                        
                        let chunked_result = crate::chunker::chunk_paired(content, &crate::chunker::ChunkOptions::default());
                        
                        let file_path_str = path.to_str().unwrap().to_string();
                        let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
                        
                        tx.execute(
                            "INSERT OR IGNORE INTO files (file_path, file_name, status, updated_at) VALUES (?1, ?2, 'indexed', ?3)",
                            (&file_path_str, &title, timestamp),
                        ).unwrap();
                        
                        let file_id: i64 = tx.query_row(
                            "SELECT file_id FROM files WHERE file_path = ?1",
                            (&file_path_str,),
                            |row| row.get(0)
                        ).unwrap();
                        
                        tx.execute("DELETE FROM chunks WHERE file_id = ?1", (file_id,)).unwrap();
                        
                        // Insert proper chunks
                        for (i, pair) in chunked_result.pairs.iter().enumerate() {
                            tx.execute(
                                "INSERT INTO chunks (file_id, chunk_index, chunk_text, embedding) VALUES (?, ?, ?, X'00')",
                                (file_id, i as i64, &pair.generation),
                            ).unwrap();
                        }
                        
                        tx.execute(
                            "INSERT OR IGNORE INTO cards (file_id, state, reps, lapses, stability, difficulty)
                             VALUES (?, 'new', 0, 0, 0.0, 0.0)",
                            (file_id,),
                        ).unwrap();
                        
                        println!("Imported: {} ({} chunks)", title, chunked_result.pairs.len());
                        imported += 1;
                }
            }
            tx.commit().unwrap();
            println!("Successfully imported {} markdown files.", imported);
        }
        "chunk-file" => {
            if args.len() < 3 {
                println!("Usage: scribo chunk-file <file_path> [--embedding|--generation|--structural|--paired]");
                return;
            }
            let file_path = &args[2];
            let mode = args.get(3).map(|s| s.as_str()).unwrap_or("--paired");
            let content = std::fs::read_to_string(file_path).expect("Could not read file");
            let default_opts = crate::chunker::ChunkOptions::default();
            
            println!("File: {}", file_path);
            
            match mode {
                "--embedding" => {
                    let chunks = crate::chunker::chunk_for_embedding(&content, &default_opts);
                    println!("Total Chunks (Embedding): {}", chunks.len());
                    for (i, chunk) in chunks.iter().enumerate() {
                        println!("\n================ CHUNK {} ================", i);
                        println!("[Tokens: {}]", crate::chunker::stages::token::count_tokens(chunk));
                        println!("{}", chunk);
                    }
                }
                "--generation" => {
                    let chunks = crate::chunker::chunk_for_generation(&content, &default_opts);
                    println!("Total Chunks (Generation): {}", chunks.len());
                    for (i, chunk) in chunks.iter().enumerate() {
                        println!("\n================ CHUNK {} ================", i);
                        println!("[Tokens: {}]", crate::chunker::stages::token::count_tokens(chunk));
                        println!("{}", chunk);
                    }
                }
                "--structural" => {
                    let struct_opts = default_opts.for_mode(crate::chunker::ChunkMode::Structural);
                    let result = crate::chunker::chunk_paired(content, &struct_opts);
                    println!("Total Chunks (Structural): {}", result.pairs.len());
                    for (i, pair) in result.pairs.iter().enumerate() {
                        println!("\n================ CHUNK {} ================", i);
                        println!("[Tokens: {}]", crate::chunker::stages::token::count_tokens(&pair.embedding));
                        println!("{}", pair.embedding);
                    }
                }
                _ => { // --paired
                    let result = crate::chunker::chunk_paired(content, &default_opts);
                    println!("Total Chunks (Paired): {}", result.pairs.len());
                    for (i, pair) in result.pairs.iter().enumerate() {
                        println!("\n================ CHUNK {} ================", i);
                        println!("[Tokens: {}]", crate::chunker::stages::token::count_tokens(&pair.generation));
                        println!("[Embedding]:\n{}\n", pair.embedding);
                        println!("[Generation]:\n{}", pair.generation);
                    }
                }
            }
        }
        "list" => {
            let mut stmt = conn.prepare("SELECT file_id, file_path FROM files WHERE is_deleted = 0").unwrap();
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            }).unwrap();
            println!("--- SCRIBO NOTES ---");
            for row in rows {
                let (id, path) = row.unwrap();
                println!("[ID: {}] {}", id, path);
            }
        }
        "search" => {
            if args.len() < 3 {
                println!("Usage: scribo search <query>");
                return;
            }
            let query = &args[2];
            let mut stmt = conn.prepare(
                "SELECT c.chunk_id, f.file_path, c.chunk_text 
                 FROM chunks_fts 
                 JOIN chunks c ON c.chunk_id = chunks_fts.rowid
                 JOIN files f ON f.file_id = c.file_id
                 WHERE chunks_fts MATCH ? LIMIT 5"
            ).unwrap();
            let rows = stmt.query_map([query], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
            }).unwrap();
            println!("Search results for '{}':", query);
            for row in rows {
                let (id, file, text) = row.unwrap();
                println!("-> {} (Chunk ID: {})\n   \"{}\"\n", file, id, text.trim());
            }
        }
        _ => {
            println!("Unknown command. Available commands: add, list, search, import-dir, chunk-file");
        }
    }
}
