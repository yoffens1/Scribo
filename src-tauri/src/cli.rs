use std::env;
use std::path::PathBuf;
use rusqlite::Connection;

pub mod import_dir;
pub mod fragment_file;
pub mod distribute;

fn get_db_path() -> PathBuf {
    let mut path = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    path.push("src-tauri");
    path.push("scribo_core.db");
    if path.exists() {
        return path;
    }
    
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

    if args.len() < 2 {
        println!("Available commands: add, add-draft, distribute, list, search, import-dir, fragment-file");
        return;
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
            import_dir::handle_import_dir(&mut conn, &args[2]);
        }
        "fragment-file" => {
            if args.len() < 3 {
                println!("Usage: scribo fragment-file <file_path> [--embedding|--generation|--structural|--paired]");
                return;
            }
            let file_path = &args[2];
            let mode = args.get(3).map(|s| s.as_str()).unwrap_or("--paired");
            fragment_file::handle_fragment_file(file_path, mode);
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
                lifecycle: Some(crate::domain::note::NoteLifecycle::Draft),
                ..Default::default()
            };
            let note_id = crate::db::repos::notes::insert(&conn, &note).unwrap();
            println!("Successfully created draft note with ID: {}", note_id.0);
        }
        "distribute" => {
            if args.len() < 3 {
                println!("Usage: scribo distribute <note_id>");
                return;
            }
            let note_id: i64 = args[2].parse().expect("Invalid note ID");
            distribute::handle_distribute(&mut conn, &db_path, note_id);
        }
        _ => {
            println!("Unknown command. Available commands: add, add-draft, distribute, list, search, import-dir, fragment-file");
        }
    }
}
