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
                "INSERT INTO notes (file_path, file_name, indexing_status, indexed_at) VALUES (?1, ?1, 'indexed', ?2)",
                (title, 12345_i64),
            ).unwrap();
            let note_id = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO fragments (note_id, fragment_index, text, embedding) VALUES (?, 0, ?, X'00')",
                (note_id, content),
            ).unwrap();
            println!("Successfully added note: '{}' (ID: {})", title, note_id);
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
                        
                        let fragmented_result = crate::fragmenter::fragment_paired(content, &crate::fragmenter::FragmentOptions::default());
                        
                        let file_path_str = path.to_str().unwrap().to_string();
                        let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
                        
                        tx.execute(
                            "INSERT OR IGNORE INTO notes (file_path, file_name, indexing_status, indexed_at) VALUES (?1, ?2, 'indexed', ?3)",
                            (&file_path_str, &title, timestamp),
                        ).unwrap();
                        
                        let note_id: i64 = tx.query_row(
                            "SELECT note_id FROM notes WHERE file_path = ?1",
                            (&file_path_str,),
                            |row| row.get(0)
                        ).unwrap();
                        
                        tx.execute("DELETE FROM fragments WHERE note_id = ?1", (note_id,)).unwrap();
                        
                        // Insert proper fragments
                        for (i, pair) in fragmented_result.pairs.iter().enumerate() {
                            tx.execute(
                                "INSERT INTO fragments (note_id, fragment_index, text, embedding) VALUES (?, ?, ?, X'00')",
                                (note_id, i as i64, &pair.generation),
                            ).unwrap();
                        }
                        
                        tx.execute(
                            "INSERT OR IGNORE INTO cards (note_id) VALUES (?)",
                            (note_id,),
                        ).unwrap();
                        let card_id: i64 = tx.last_insert_rowid();
                        tx.execute(
                            "INSERT OR IGNORE INTO schedules (target_type, target_id, state)
                             VALUES ('card', ?, 'new')",
                            (card_id,),
                        ).unwrap();
                        
                        println!("Imported: {} ({} fragments)", title, fragmented_result.pairs.len());
                        imported += 1;
                }
            }
            tx.commit().unwrap();
            println!("Successfully imported {} markdown notes.", imported);
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
            let mut stmt = conn.prepare("SELECT note_id, file_path FROM notes WHERE is_deleted = 0").unwrap();
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
                "SELECT c.fragment_id, f.file_path, c.text 
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
                let (id, file, text) = row.unwrap();
                println!("-> {} (Fragment ID: {})\n   \"{}\"\n", file, id, text.trim());
            }
        }
        _ => {
            println!("Unknown command. Available commands: add, list, search, import-dir, fragment-file");
        }
    }
}
