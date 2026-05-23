mod commands;
mod error;
pub mod chunker;
pub mod db;
pub mod ai;
pub mod refinery;
pub mod filesearch;
pub mod translation;
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
            commands::files::files_get_by_path,
            commands::files::files_upsert_indexing,
            commands::files::files_mark_indexed,
            commands::files::files_mark_failed,
            commands::files::files_insert_failed,
            commands::files::files_soft_delete,
            commands::files::files_restore,
            commands::files::files_rename,
            commands::files::files_count_chunks,
            commands::files::files_hard_delete,
            commands::files::files_get_all,
            commands::files::files_get_by_source_file_id,
            commands::files::files_insert_minimal,
            commands::files::files_sync_upsert,
            commands::files::files_update_content_with_diff,
            commands::chunks::chunks_delete_by_file_id,
            commands::chunks::chunks_insert,
            commands::chunks::chunks_get_by_file_path,
            commands::chunks::chunks_get_all,
            commands::chunks::chunks_get_by_file_name,
            commands::chunks::chunks_search,
            commands::chunks::chunks_vector_search,
            commands::cards::cards_insert_ignore,
            commands::cards::cards_review_fsrs,
            commands::chunker::chunk_text_paired,
            commands::chunker::count_text_tokens,
            commands::ai::ai_generate,
            commands::ai::ai_generate_embeddings,
            commands::refinery::refinery_run_pipeline,
            commands::search::filesearch_fuzzy,
            commands::search::translation_translate,
            commands::search::retrieval_detect_language,
            commands::search::retrieval_is_english,
            commands::search::retrieval_query,
            commands::search::retrieval_fetch,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

