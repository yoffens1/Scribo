use tauri::State;
use crate::DbState;
use crate::error::AppError;
use crate::db::repos::cards;
use crate::domain::card::{CardReviewParams, ReviewResult};

#[tauri::command]
pub fn cards_insert_ignore(state: State<'_, DbState>, file_id: i64) -> Result<(), AppError> {
    state.with_conn(|conn| cards::insert_ignore(conn, file_id))
}

#[tauri::command]
pub fn cards_review_fsrs(
    state: State<'_, DbState>,
    params: CardReviewParams,
) -> Result<ReviewResult, AppError> {
    let _w = state.write_lock.lock();
    state.with_conn(|conn| cards::review_fsrs(conn, params))
}
