//! # Fragmenter Commands
//!
//! Exposes pure AST-chunking utilities to the frontend without requiring database state.

use crate::AppError;
use crate::fragmenter::{FragmentConfig, FragmenterPairedResult};

/// Chunks raw markdown into an array of sections (headings) and fragments (text content).
/// Uses default packing thresholds.
#[tauri::command]
pub fn fragment_text_paired(content: String) -> Result<FragmenterPairedResult, AppError> {
    Ok(crate::fragmenter::fragment_paired(content, &FragmentConfig::default()))
}

/// Returns the token count for a given text using the tiktoken `cl100k_base` encoding.
#[tauri::command]
pub fn count_text_tokens(text: String) -> usize {
    crate::fragmenter::token::count_tokens(&text)
}
