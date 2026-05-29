//! # Reviewer Commands
//!
//! Tauri commands for the Flashcard Spaced Repetition System (FSRS).
//! Interfaces with the stateless `ReviewerService` stored in `DbState`.

use tauri::State;
use crate::db::DbState;
use crate::domain::{Rating, Schedule, ScheduleId};
use crate::services::reviewer::ReviewResult;

/// Retrieves the next `limit` flashcards/notes that are due for review.
#[tauri::command]
pub async fn reviewer_get_due(
    state: State<'_, DbState>,
    limit: i64,
) -> Result<Vec<Schedule>, String> {
    state.with_conn(|conn| {
        state.reviewer.get_due_reviews(conn, limit)
    }).map_err(|e| e.to_string())
}

/// Submits a rating (Again=1, Hard=2, Good=3, Easy=4) for a schedule.
/// Computes the new FSRS stability/difficulty and schedules the next review date.
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

/// Manually overrides a note's review schedule to force it to appear in `days`.
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

/// Fetches a specific card by ID and formats its custom front/back/cloze into a `RenderedCard`.
#[tauri::command]
pub async fn reviewer_get_card(
    state: State<'_, DbState>,
    card_id: i64,
) -> Result<crate::domain::card::RenderedCard, String> {
    state.with_conn(|conn| {
        let card = crate::db::repos::cards::find_by_id(conn, crate::domain::CardId(card_id))
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Card not found".to_string())?;

        let section = match card.section_id {
            Some(sid) => crate::db::repos::sections::find_by_id(conn, sid)
                .map_err(|e| e.to_string())?,
            None => None,
        };

        let note = crate::db::repos::notes::get_by_id(conn, card.note_id.0)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Note not found".to_string())?;

        let rendered = card.render(section.as_ref(), note.id, note.title, note.path_cached);
        Ok(rendered)
    }).map_err(|e: crate::AppError| e.to_string())
}

/// Automatically upgrades a basic heading-type card to a Q&A card.
/// Uses the LLM to generate an active recall question based on the section's raw text.
#[tauri::command]
pub async fn reviewer_upgrade_card_front_with_ai(
    app: tauri::AppHandle,
    state: State<'_, DbState>,
    config: crate::ai::LlmConfig,
    card_id: i64,
) -> Result<String, String> {
    let service = crate::ai::LlmService::new(config, Some(app));
    
    let (card, section_text) = state.with_conn(|conn| {
        let card = crate::db::repos::cards::find_by_id(conn, crate::domain::CardId(card_id))
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Card not found".to_string())?;

        let section_text = match card.section_id {
            Some(sid) => {
                let section = crate::db::repos::sections::find_by_id(conn, sid)
                    .map_err(|e| e.to_string())?
                    .ok_or_else(|| "Section not found".to_string())?;
                section.text_raw
            }
            None => {
                card.last_section_snapshot.clone().unwrap_or_default()
            }
        };

        Ok((card, section_text))
    }).map_err(|e: crate::AppError| e.to_string())?;

    let prompt = format!(
        "Based on the following text, generate a single, concise active recall question. \
         The question should be direct and test the reader's understanding of the core concepts in the text.\n\n\
         Text:\n```\n{}\n```\n\n\
         Output only the question and nothing else. Do not include any greeting, markdown formatting (like bolding), or introductory phrases.",
        section_text
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
            Some(&section_text),
            card.status,
            card.last_section_snapshot.as_deref(),
        )?;

        conn.execute(
            "UPDATE cards SET card_type = 'qa' WHERE card_id = ?",
            rusqlite::params![card.id.0],
        )?;

        Ok(())
    }).map_err(|e: crate::AppError| e.to_string())?;

    Ok(generated_question)
}

/// Returns a grouped count of due cards aggregated up the note hierarchy.
/// Used to display notification badges on folders/notes in the sidebar.
#[tauri::command]
pub async fn reviewer_get_hierarchical_due_counts(
    state: State<'_, DbState>,
) -> Result<Vec<crate::domain::NoteDueCount>, String> {
    state.with_conn(|conn| {
        state.reviewer.get_hierarchical_due_counts(conn)
    }).map_err(|e| e.to_string())
}

/// Returns a tree structure matching the vault directory tree, filtered to show
/// only notes/folders that contain at least one due card.
#[tauri::command]
pub async fn reviewer_get_repeat_mode_tree(
    state: State<'_, DbState>,
) -> Result<Vec<crate::domain::RepeatModeNode>, String> {
    state.with_conn(|conn| {
        state.reviewer.get_repeat_mode_tree(conn)
    }).map_err(|e| e.to_string())
}


