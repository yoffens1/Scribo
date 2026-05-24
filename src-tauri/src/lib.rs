mod commands;
mod error;
pub mod cli;
pub mod domain;
pub mod services;
pub mod fragmenter;
pub mod db;
pub mod ai;
pub mod refinery;
pub mod retrieval;
pub mod logging;

pub use error::AppError;
pub use db::DbState;

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
            commands::refinery::refinery_run_pipeline,
            commands::search::notesearch_fuzzy,
            commands::search::translation_translate,
            commands::search::retrieval_detect_language,
            commands::search::retrieval_is_english,
            commands::search::retrieval_query,
            commands::search::retrieval_fetch,
            commands::reviewer::reviewer_get_due,
            commands::reviewer::reviewer_rate,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

