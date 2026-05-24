use crate::services::filesearch::{FuzzySearch, SearchHit};
use crate::ai::{LlmConfig, LlmService, Translator};
use crate::retrieval::{detect_language, is_english, retrieve, fetch, RetrievalConfig, SearchResult as RetSearchResult, RetrieveOptions, FetchQuery, FetchResult};
use crate::error::AppError;
use crate::db::DbState;
use tauri::State;
use std::sync::Arc;

#[tauri::command]
pub async fn retrieval_query(
    query: String,
    query_embedding: Option<Vec<f32>>,
    config: RetrievalConfig,
    options: Option<RetrieveOptions>,
    state: State<'_, DbState>,
) -> Result<Vec<RetSearchResult>, AppError> {
    let opts = options.unwrap_or(RetrieveOptions {
        top_k: None,
        filters: None,
    });
    retrieve(&state, &query, query_embedding.as_deref(), &config, &opts).await
}

#[tauri::command]
pub async fn filesearch_fuzzy(
    query: String,
    files: Vec<String>,
    limit: usize,
) -> Result<Vec<SearchHit>, AppError> {
    // Note: instantiating FuzzySearch on every request might be slightly slow if `files` is huge,
    // but for now this perfectly matches the TS implementation.
    let search = FuzzySearch::new(files);
    let results = search.search(&query, limit);
    Ok(results)
}

#[tauri::command]
pub async fn translation_translate(
    text: String,
    target_lang: String,
    llm_config: LlmConfig,
) -> Result<String, AppError> {
    let llm_service = Arc::new(LlmService::new(llm_config, None));
    let translator = Translator::new(llm_service);
    translator.translate(&text, &target_lang).await.map_err(|e| AppError::Other(e))
}

#[tauri::command]
pub fn retrieval_detect_language(text: String) -> Result<Option<String>, AppError> {
    Ok(detect_language(&text))
}

#[tauri::command]
pub fn retrieval_is_english(text: String) -> Result<bool, AppError> {
    Ok(is_english(&text))
}

#[tauri::command]
pub async fn retrieval_fetch(
    query: FetchQuery,
    state: State<'_, DbState>,
) -> Result<Vec<FetchResult>, AppError> {
    fetch(&state, &query)
}
