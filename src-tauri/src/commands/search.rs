//! # Search Commands
//!
//! Tauri commands for semantic search (RAG), fuzzy string search, and related utilities.

use crate::services::notesearch::{FuzzySearch, SearchHit};
use crate::ai::{LlmConfig, LlmService, Translator};
use crate::retrieval::{detect_language, is_english, retrieve, fetch, RetrievalConfig, SearchResult as RetSearchResult, RetrieveOptions, FetchQuery, FetchResult};
use crate::error::AppError;
use crate::db::DbState;
use tauri::State;
use std::sync::Arc;

/// Runs the semantic retrieval pipeline. Supports vector search, FTS5 keyword search,
/// HyDE (Hypothetical Document Embeddings), and LLM-based reranking.
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
        target_level: None,
    });
    retrieve(&state, &query, query_embedding.as_deref(), &config, &opts).await
}

/// Simple trigram-based fuzzy string matcher.
/// Used by the frontend for fast in-memory note title filtering.
#[tauri::command]
pub async fn notesearch_fuzzy(
    query: String,
    notes: Vec<String>,
    limit: usize,
) -> Result<Vec<SearchHit>, AppError> {
    // Note: instantiating FuzzySearch on every request might be slightly slow if `notes` is huge,
    // but for now this perfectly matches the TS implementation.
    let search = FuzzySearch::new(notes);
    let results = search.search(&query, limit);
    Ok(results)
}

/// Uses the LLM to translate a block of text into `target_lang`.
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

/// Uses `whatlang` to detect the ISO-639-1 language code of a string.
#[tauri::command]
pub fn retrieval_detect_language(text: String) -> Result<Option<String>, AppError> {
    Ok(detect_language(&text))
}

/// Fast check if text is predominantly English (used to skip translation steps).
#[tauri::command]
pub fn retrieval_is_english(text: String) -> Result<bool, AppError> {
    Ok(is_english(&text))
}

/// Resolves a pre-computed batch of search results into full fragment payloads.
/// Used for paginated loading of RAG contexts.
#[tauri::command]
pub async fn retrieval_fetch(
    query: FetchQuery,
    state: State<'_, DbState>,
) -> Result<Vec<FetchResult>, AppError> {
    fetch(&state, &query)
}
