use crate::AppError;
use crate::chunker::{ChunkOptions, types::ChunkerResult};

#[tauri::command]
pub fn chunk_text_paired(content: String) -> Result<ChunkerResult, AppError> {
    Ok(crate::chunker::chunk_paired(content, &ChunkOptions::default()))
}

#[tauri::command]
pub fn count_text_tokens(text: String) -> usize {
    crate::chunker::token::count_tokens(&text)
}
