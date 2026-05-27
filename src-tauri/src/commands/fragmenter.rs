use crate::AppError;
use crate::fragmenter::{FragmentOptions, types::FragmenterResult};

#[tauri::command]
pub fn fragment_text_paired(content: String) -> Result<FragmenterResult, AppError> {
    Ok(crate::fragmenter::fragment_paired(content, &FragmentOptions::default()))
}

#[tauri::command]
pub fn count_text_tokens(text: String) -> usize {
    crate::fragmenter::stages::token::count_tokens(&text)
}
