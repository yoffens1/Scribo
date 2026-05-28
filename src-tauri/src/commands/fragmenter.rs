use crate::AppError;
use crate::fragmenter::{FragmentConfig, FragmenterPairedResult};

#[tauri::command]
pub fn fragment_text_paired(content: String) -> Result<FragmenterPairedResult, AppError> {
    Ok(crate::fragmenter::fragment_paired(content, &FragmentConfig::default()))
}

#[tauri::command]
pub fn count_text_tokens(text: String) -> usize {
    crate::fragmenter::token::count_tokens(&text)
}
