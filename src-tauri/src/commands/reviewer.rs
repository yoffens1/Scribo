use tauri::State;
use crate::db::DbState;
use crate::domain::{Rating, Schedule, ScheduleId};
use crate::services::reviewer::ReviewResult;

#[tauri::command]
pub async fn reviewer_get_due(
    state: State<'_, DbState>,
    limit: i64,
) -> Result<Vec<Schedule>, String> {
    state.with_conn(|conn| {
        state.reviewer.get_due_reviews(conn, limit)
    }).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn reviewer_rate(
    state: State<'_, DbState>,
    schedule_id: i64,
    rating: i32,
) -> Result<ReviewResult, String> {
    let r = Rating::from_i32(rating).ok_or("Invalid rating")?;
    state.with_write(|conn| {
        state.reviewer.rate_review(conn, ScheduleId(schedule_id), r)
    }).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn reviewer_schedule_note_in_days(
    state: State<'_, DbState>,
    note_id: i64,
    days: i64,
) -> Result<Schedule, String> {
    state.with_write(|conn| {
        state.reviewer.schedule_note_in_days(conn, crate::domain::NoteId(note_id), days)
    }).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn reviewer_get_card(
    state: State<'_, DbState>,
    card_id: i64,
) -> Result<crate::domain::card::RenderedCard, String> {
    state.with_conn(|conn| {
        let card = crate::db::repos::cards::find_by_id(conn, crate::domain::CardId(card_id))
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Card not found".to_string())?;

        let section = crate::db::repos::sections::find_by_id(conn, card.section_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Section not found".to_string())?;

        let note = crate::db::repos::notes::get_by_id(conn, section.note_id.0)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Note not found".to_string())?;

        let rendered = card.render(&section, note.id, note.title, note.path_cached);
        Ok(rendered)
    }).map_err(|e: crate::AppError| e.to_string())
}

#[tauri::command]
pub async fn reviewer_upgrade_card_front_with_ai(
    app: tauri::AppHandle,
    state: State<'_, DbState>,
    config: crate::ai::LlmConfig,
    card_id: i64,
) -> Result<String, String> {
    let service = crate::ai::LlmService::new(config, Some(app));
    
    let (card, section) = state.with_conn(|conn| {
        let card = crate::db::repos::cards::find_by_id(conn, crate::domain::CardId(card_id))
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Card not found".to_string())?;

        let section = crate::db::repos::sections::find_by_id(conn, card.section_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Section not found".to_string())?;

        Ok((card, section))
    }).map_err(|e: crate::AppError| e.to_string())?;

    let prompt = format!(
        "Based on the following text, generate a single, concise active recall question. \
         The question should be direct and test the reader's understanding of the core concepts in the text.\n\n\
         Text:\n```\n{}\n```\n\n\
         Output only the question and nothing else. Do not include any greeting, markdown formatting (like bolding), or introductory phrases.",
        section.text_raw
    );

    let messages = vec![crate::ai::types::Message {
        role: "user".to_string(),
        content: prompt,
    }];

    let response = service.generate_messages(messages).await?;
    let generated_question = response.text.trim().to_string();

    state.with_write(|conn| {
        crate::db::repos::cards::update(
            conn,
            card.id.0,
            Some(&generated_question),
            Some(&section.text_raw),
            card.is_suspended,
            false,
        )?;

        conn.execute(
            "UPDATE cards SET card_type = 'qa' WHERE card_id = ?",
            rusqlite::params![card.id.0],
        )?;

        Ok(())
    }).map_err(|e: crate::AppError| e.to_string())?;

    Ok(generated_question)
}

#[tauri::command]
pub async fn reviewer_get_hierarchical_due_counts(
    state: State<'_, DbState>,
) -> Result<Vec<crate::domain::NoteDueCount>, String> {
    state.with_conn(|conn| {
        state.reviewer.get_hierarchical_due_counts(conn)
    }).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn reviewer_get_repeat_mode_tree(
    state: State<'_, DbState>,
) -> Result<Vec<crate::domain::RepeatModeNode>, String> {
    state.with_conn(|conn| {
        state.reviewer.get_repeat_mode_tree(conn)
    }).map_err(|e| e.to_string())
}


