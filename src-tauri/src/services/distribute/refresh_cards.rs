use crate::error::AppError;
use crate::ai::{LlmService, extract_json_payload};
use crate::DbState;
use std::sync::Arc;

pub async fn refresh_stale_cards_for_notes(
    state: &DbState,
    note_ids: &[i64],
    llm_service: &Arc<LlmService>,
) -> Result<(), AppError> {
    let stale_cards = state.with_conn(|conn| {
        let mut stale = Vec::new();
        for &note_id in note_ids {
            let cards = crate::db::repos::cards::list_by_note(conn, note_id)?;
            for card in cards {
                if card.status == crate::domain::card::CardLifecycle::Stale {
                    if let Some(sid) = card.section_id {
                        if let Some(section) = crate::db::repos::sections::find_by_id(conn, sid)? {
                            stale.push((card, section));
                        }
                    }
                }
            }
        }
        Ok(stale)
    })?;

    for (card, section) in stale_cards {
        let note_path = state.with_conn(|conn| {
            if let Some(note) = crate::db::repos::notes::get_by_id(conn, section.note_id.0)? {
                Ok(note.path_cached.clone())
            } else {
                Ok("note.md".to_string())
            }
        })?;

        let prompt_messages = crate::ai::prompts::build_atomize_prompt(&section.text_raw, &note_path);
        
        if let Ok(response) = llm_service.generate_messages(prompt_messages).await {
            #[derive(serde::Deserialize)]
            struct AtomizeResponse {
                #[serde(rename = "questionHeading")]
                question_heading: String,
            }

            let mut custom_front = None;
            if let Some(json_str) = extract_json_payload(&response.text) {
                if let Ok(parsed) = serde_json::from_str::<AtomizeResponse>(&json_str) {
                    let clean_front = parsed.question_heading.trim_start_matches("##").trim().to_string();
                    custom_front = Some(clean_front);
                }
            }

            let front = custom_front.unwrap_or_else(|| {
                format!("What are the key points of {}?", section.heading.as_deref().unwrap_or("this section"))
            });

            state.with_write(|conn| {
                crate::db::repos::cards::update(
                    conn,
                    card.id.0,
                    Some(&front),
                    Some(&section.text_raw),
                    crate::domain::card::CardLifecycle::Fresh,
                    None,
                )?;
                conn.execute(
                    "UPDATE cards SET card_type = 'qa' WHERE card_id = ?",
                    rusqlite::params![card.id.0],
                )?;
                Ok(())
            })?;
        }
    }

    Ok(())
}
