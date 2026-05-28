//! # Scribo Command Line Interface (CLI)
//!
//! Provides CLI parsing, subcommand definitions, database configuration,
//! and routing of commands to their respective CLI handlers.
//! The CLI operates directly on the SQLite database without starting Tauri.

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use rusqlite::Connection;

pub mod import_dir;
pub mod fragment_file;
pub mod distribute;
pub mod query;
pub mod reindex;

/// Command line interface parser for Scribo.
#[derive(Parser, Debug)]
#[command(name = "scribo", about = "Scribo CLI", version)]
struct Cli {
    /// Subcommand to execute.
    #[command(subcommand)]
    command: Commands,
}

/// Available subcommands for the CLI.
#[derive(Subcommand, Debug)]
#[command(rename_all = "kebab-case")]
enum Commands {
    /// Show current database status and check paths.
    Status,
    /// Add a new active note directly to the vault.
    Add {
        /// Title of the note.
        title: String,
        /// Markdown content of the note.
        content: String,
    },
    /// Create a new draft note (used as incoming/inbox for drafts).
    AddDraft {
        /// Markdown content of the draft note.
        content: String,
    },
    /// Distribute thematic sections of a draft note to existing active notes.
    Distribute {
        /// Identifier of the draft note.
        note_id: i64,
    },
    /// List all active notes in the database.
    List,
    /// Search fragment texts using SQLite FTS5 search.
    Search {
        /// The search term query.
        query: String,
    },
    /// Import an entire directory of markdown files and index them with local embeddings.
    ImportDir {
        /// Path to the directory to import.
        path: String,
    },
    /// Run the markdown fragmenter on a specific file and output structural sections.
    FragmentFile {
        /// Path to the markdown file to inspect.
        file_path: String,
        /// Parsing mode (`--embedding`, `--generation`, `--structural`, `--paired`).
        #[arg(default_value = "--paired")]
        mode: String,
    },
    /// Hybrid retrieval (FTS5 + vector + RRF). Embeds the query locally.
    Query {
        /// Search query text.
        query: String,
        /// Number of results to return.
        #[arg(short = 'k', long, default_value_t = 5)]
        top_k: usize,
        /// Search mode: `hybrid` (default), `embedding`, or `keyword`.
        #[arg(short = 'm', long, default_value = "hybrid")]
        mode: String,
        /// Enable HyDE (Hypothetical Document Embeddings) using default LLM.
        #[arg(long)]
        hyde: bool,
        /// Enable Listwise reranking using default LLM.
        #[arg(long)]
        rerank: bool,
        /// Enable Synonym expansion using static dictionary.
        #[arg(long)]
        expand: bool,
        /// Hard threshold for RRF/similarity score.
        #[arg(long, default_value_t = 0.005)]
        min_score: f32,
    },
    /// Re-calculate and update embeddings for all fragments in the database.
    Reindex,
}

/// Resolves the file path to the Scribo SQLite database.
/// Checks the local workspace `src-tauri` directory first, falls back to the system data directory,
/// and defaults to the current working directory.
fn get_db_path() -> PathBuf {
    let mut path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
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
    let mut path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    path.push("scribo_core.db");
    path
}

/// Entrypoint for handling commands from the CLI.
/// Connects to the SQLite database, checks schema migrations, and routes the subcommand.
pub fn handle_cli(args: Vec<String>) {
    let cli = match Cli::try_parse_from(args) {
        Ok(c) => c,
        Err(err) => {
            err.exit();
        }
    };

    let db_path = get_db_path();
    let mut conn = Connection::open(&db_path).expect("Failed to open database");

    if let Err(e) = crate::db::schema::initialize_schema(&mut conn) {
        eprintln!("Warning: Failed to initialize schema: {}", e);
    }

    match cli.command {
        Commands::Status => {
            println!("Database path: {}", db_path.display());
            println!("Ready to accept commands.");
        }
        Commands::Add { title, content } => {
            let note = crate::domain::note::NewNote {
                title: title.clone(),
                content: content.clone(),
                ..Default::default()
            };
            let note_id = crate::db::repos::notes::insert(&conn, &note).unwrap();
            println!("Successfully added note: '{}' (ID: {})", title, note_id.0);
        }
        Commands::ImportDir { path } => {
            import_dir::handle_import_dir(&mut conn, &path);
        }
        Commands::FragmentFile { file_path, mode } => {
            fragment_file::handle_fragment_file(&file_path, &mode);
        }
        Commands::List => {
            let notes = crate::db::repos::notes::get_all(&conn).unwrap();
            println!("--- SCRIBO NOTES ---");
            for note in notes {
                println!("[ID: {}] {}", note.note_id.0, note.title);
            }
        }
        Commands::Search { query } => {
            let hits = crate::db::repos::fragments::search(&conn, &query, 5).unwrap();
            println!("Search results for '{}':", query);
            for scored in hits {
                let hit = scored.hit;
                println!(
                    "-> {} (Fragment ID: {})\n   \"{}\"\n",
                    hit.note_title.unwrap_or_else(|| "Untitled".to_string()),
                    hit.fragment_id.0,
                    hit.text.trim()
                );
            }
        }
        Commands::AddDraft { content } => {
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
        Commands::Distribute { note_id } => {
            distribute::handle_distribute(&mut conn, &db_path, note_id);
        }
        Commands::Query {
            query,
            top_k,
            mode,
            hyde,
            rerank,
            expand,
            min_score,
        } => {
            use crate::retrieval::types::RetrievalMode;
            let mode = match mode.as_str() {
                "embedding" | "vector" => RetrievalMode::Embedding,
                "keyword"   | "fts"    => RetrievalMode::Keyword,
                _                      => RetrievalMode::Hybrid,
            };
            query::handle_query(
                &db_path,
                &query,
                top_k,
                mode,
                hyde,
                rerank,
                expand,
                min_score,
            );
        }
        Commands::Reindex => {
            reindex::handle_reindex(&mut conn);
        }
    }
}
