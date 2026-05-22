#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::env;
use std::path::PathBuf;
use rusqlite::Connection;

fn get_db_path() -> PathBuf {
    // Для удобства разработки создаем базу прямо в текущей директории репозитория
    let mut path = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    path.push("scribo_core.db");
    path
}

fn handle_cli(args: Vec<String>) {
    let db_path = get_db_path();
    let mut conn = Connection::open(&db_path).expect("Failed to open database");

    // Ensure the database is initialized before CLI actions
    if let Err(e) = scribo_lib::schema::initialize_schema(&mut conn) {
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
                rusqlite::params![title, 12345],
            ).unwrap();
            let file_id = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO chunks (file_id, chunk_index, chunk_text, embedding) VALUES (?, 0, ?, X'00')",
                rusqlite::params![file_id, content],
            ).unwrap();
            println!("Successfully added note: '{}' (ID: {})", title, file_id);
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
            println!("Unknown command. Available commands: add, list, search");
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 {
        handle_cli(args);
    } else {
        scribo_lib::run();
    }
}
