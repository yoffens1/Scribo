mod commands;
mod error;
pub mod indexer;
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
            commands::db::core::db_initialize,
            commands::db::core::db_close,
            commands::db::core::db_vacuum,
            commands::db::core::db_optimize,
            commands::db::files::files_get_by_path,
            commands::db::files::files_upsert_indexing,
            commands::db::files::files_mark_indexed,
            commands::db::files::files_mark_failed,
            commands::db::files::files_insert_failed,
            commands::db::files::files_soft_delete,
            commands::db::files::files_restore,
            commands::db::files::files_rename,
            commands::db::files::files_count_chunks,
            commands::db::files::files_hard_delete,
            commands::db::files::files_get_all,
            commands::db::files::files_get_by_source_file_id,
            commands::db::files::files_insert_minimal,
            commands::db::files::files_sync_upsert,
            commands::db::files::files_update_content_with_diff,
            commands::db::chunks::chunks_delete_by_file_id,
            commands::db::chunks::chunks_insert,
            commands::db::chunks::chunks_get_by_file_path,
            commands::db::chunks::chunks_get_all,
            commands::db::chunks::chunks_get_by_file_name,
            commands::db::chunks::chunks_search,
            commands::db::chunks::chunks_vector_search,
            commands::srs::cards::cards_insert_ignore,
            commands::srs::cards::cards_review_fsrs,
            commands::ai::chunker::chunk_text_paired,
            commands::ai::chunker::count_text_tokens,
            commands::ai::models::ai_generate,
            commands::ai::models::ai_generate_embeddings,
            commands::ai::models::ai_list_local_models,
            commands::ai::models::ai_local_unload_model,
            commands::ai::refinery::refinery_run_pipeline,
            commands::search::retrieval::filesearch_fuzzy,
            commands::search::retrieval::translation_translate,
            commands::search::retrieval::retrieval_detect_language,
            commands::search::retrieval::retrieval_is_english,
            commands::search::retrieval::retrieval_query,
            commands::search::retrieval::retrieval_fetch,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

