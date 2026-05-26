use tauri::State;
use crate::db::DbState;
use crate::domain::{Rating, Schedule, ScheduleId};
use crate::services::reviewer::ReviewResult;

#[tauri::command]
pub async fn reviewer_get_due(
    state: State<'_, DbState>,
    limit: i64,
) -> Result<Vec<Schedule>, String> {
    state.reviewer.get_due_reviews(limit).await
}

#[tauri::command]
pub async fn reviewer_rate(
    state: State<'_, DbState>,
    schedule_id: i64,
    rating: i32,
) -> Result<ReviewResult, String> {
    let r = Rating::from_i32(rating).ok_or("Invalid rating")?;
    state.reviewer.rate_review(ScheduleId(schedule_id), r).await
}

#[tauri::command]
pub async fn reviewer_schedule_note_in_days(
    state: State<'_, DbState>,
    note_id: i64,
    days: i64,
) -> Result<Schedule, String> {
    state.reviewer.schedule_note_in_days(crate::domain::NoteId(note_id), days).await
}
