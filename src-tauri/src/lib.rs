//! # Scribo Library Core
//!
//! Provides the primary library implementation for the Scribo application.
//! It orchestrates submodules for AI services, database, CLI commands, markdown parsing (fragmenter),
//! retrieval pipelines, Spaced Repetition System (SRS) review, and logging.
//! It also houses the main entry point to register commands and run the Tauri application.

pub mod core;
pub mod entrypoints;
pub mod domain;
pub mod services;
pub mod fragmenter;
pub mod db;
pub mod ai;
pub mod retrieval;
pub mod logging;

pub use crate::entrypoints::cli;
pub use crate::entrypoints::tauri as commands;

pub use crate::core::{constants, lang, error};
pub use crate::core::error::AppError;
pub use db::DbState;

/// Initializes and launches the Tauri application.
/// Sets up loggers, manages shared database state, registers IPC handlers,
/// and starts the main event loop.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            if let Ok(guard) = logging::setup_logger(app.handle()) {
                use tauri::Manager;
                app.manage(guard);
            }
            Ok(())
        })
        .manage(DbState::new())
        .invoke_handler(tauri::generate_handler![
            crate::logging::log_event,
            commands::db::db_initialize,
            commands::db::db_close,
            commands::db::db_vacuum,
            commands::db::db_optimize,
            commands::ai::ai_generate,
            commands::ai::ai_generate_embeddings,
            commands::ai::ai_list_local_models,
            commands::ai::ai_local_unload_model,
            commands::search::notesearch_fuzzy,
            commands::search::translation_translate,
            commands::search::retrieval_detect_language,
            commands::search::retrieval_is_english,
            commands::search::retrieval_query,
            commands::search::retrieval_fetch,
            commands::reviewer::reviewer_get_due,
            commands::reviewer::reviewer_rate,
            commands::reviewer::reviewer_schedule_note_in_days,
            commands::reviewer::reviewer_upgrade_card_front_with_ai,
            commands::reviewer::reviewer_get_card,
            commands::reviewer::reviewer_get_hierarchical_due_counts,
            commands::reviewer::reviewer_get_repeat_mode_tree,
            commands::distribute::distribute_analyze_draft,
            commands::distribute::distribute_apply_plan,
            commands::fragmenter::fragment_text_paired,
            commands::fragmenter::count_text_tokens,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

