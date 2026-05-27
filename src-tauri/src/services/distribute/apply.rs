use rusqlite::Connection;
use crate::error::AppError;
use crate::domain::distribute::DraftDistributionPlan;
use crate::domain::note::{NoteId, NewNote};

pub fn apply_distribution(
    conn: &mut Connection,
    plan: DraftDistributionPlan,
) -> Result<(), AppError> {
    // Pre-serialize plan before consuming plan.chunks
    let result_json = serde_json::to_string(&plan).unwrap_or_default();

    // 1. First apply all 'append' actions
    for chunk in &plan.chunks {
        if chunk.recommendation.action == "append" {
            if let Some(target_id) = chunk.recommendation.target_note_id {
                let target_note = crate::db::repos::notes::get_by_id(conn, target_id)?
                    .ok_or_else(|| AppError::Other(format!("Target note not found: {}", target_id)))?;
                
                let separator = format!(
                    "\n\n<!-- imported from draft #{} on {} -->\n",
                    plan.draft_id,
                    chrono::Utc::now().to_rfc3339()
                );
                let new_content = if target_note.content.is_empty() {
                    chunk.text.clone()
                } else {
                    format!("{}{}{}", target_note.content, separator, chunk.text)
                };
                
                crate::db::repos::notes::update_content_with_diff(conn, target_id, &new_content)?;
            }
        }
    }

    // 2. Insert new child notes resolving temporary negative parent IDs
    let mut inserted_notes = std::collections::HashMap::new();
    let mut pending_creations: Vec<crate::domain::distribute::ChunkDistributionPlan> = plan.chunks.into_iter()
        .filter(|c| c.recommendation.action == "create_child")
        .collect();

    let mut progressed = true;
    while !pending_creations.is_empty() && progressed {
        progressed = false;
        let mut next_pending = Vec::new();

        for chunk in pending_creations {
            let parent_id_opt = chunk.recommendation.parent_note_id;
            
            let can_insert = match parent_id_opt {
                Some(pid) if pid < 0 => {
                    let parent_chunk_idx = (-pid - 1) as usize;
                    inserted_notes.contains_key(&parent_chunk_idx)
                }
                _ => true,
            };

            if can_insert {
                let title = chunk.recommendation.new_note_title.clone()
                    .unwrap_or_else(|| chunk.suggested_title.clone());

                let parent_id = match parent_id_opt {
                    Some(pid) if pid < 0 => {
                        let parent_chunk_idx = (-pid - 1) as usize;
                        inserted_notes.get(&parent_chunk_idx).map(|id| NoteId(*id))
                    }
                    Some(pid) => Some(NoteId(pid)),
                    None => None,
                };

                let new_note = NewNote {
                    title,
                    content: chunk.text,
                    parent_note_id: parent_id,
                    is_draft: false,
                    ..Default::default()
                };
                let new_id = crate::db::repos::notes::insert(conn, &new_note)?;
                inserted_notes.insert(chunk.chunk_index, new_id.0);
                progressed = true;
            } else {
                next_pending.push(chunk);
            }
        }

        pending_creations = next_pending;
    }

    // Safe fallback if circular dependency occurs
    for chunk in pending_creations {
        let title = chunk.recommendation.new_note_title.clone()
            .unwrap_or_else(|| chunk.suggested_title.clone());

        let new_note = NewNote {
            title,
            content: chunk.text,
            parent_note_id: None,
            is_draft: false,
            ..Default::default()
        };
        crate::db::repos::notes::insert(conn, &new_note)?;
    }

    // Move original draft note to archive
    crate::db::repos::notes::archive_note(conn, plan.draft_id)?;

    // Log the applied distribution run using pre-serialized result_json
    let updated = conn.execute(
        "UPDATE distribution_runs 
         SET status = 'applied', result_json = ?, applied_at = strftime('%s','now') 
         WHERE draft_id = ? AND status = 'analyzed'",
        rusqlite::params![result_json, plan.draft_id],
    )?;
    if updated == 0 {
        conn.execute(
            "INSERT INTO distribution_runs (draft_id, plan_json, result_json, generator_version, status, created_at, applied_at)
             VALUES (?, ?, ?, 'v1', 'applied', strftime('%s','now'), strftime('%s','now'))",
            rusqlite::params![plan.draft_id, result_json, result_json],
        )?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::distribute::{ChunkDistributionPlan, LlmRecommendation};

    #[test]
    fn test_apply_distribution_logic() {
        let manager = r2d2_sqlite::SqliteConnectionManager::memory();
        let pool = r2d2::Pool::builder().max_size(1).build(manager).unwrap();
        let mut conn = pool.get().unwrap();
        crate::db::schema::initialize_schema(&mut conn).unwrap();

        // 1. Insert a draft note that will be distributed
        let draft = NewNote {
            title: "Quick draft".to_string(),
            content: "Draft content to distribute".to_string(),
            is_draft: true,
            ..Default::default()
        };
        let draft_id = crate::db::repos::notes::insert(&conn, &draft).unwrap();

        // 2. Insert a target note to append to
        let target = NewNote {
            title: "Math".to_string(),
            content: "Initial math content".to_string(),
            is_draft: false,
            ..Default::default()
        };
        let target_id = crate::db::repos::notes::insert(&conn, &target).unwrap();

        // 3. Construct a distribution plan
        let plan = DraftDistributionPlan {
            draft_id: draft_id.0,
            chunks: vec![
                ChunkDistributionPlan {
                    chunk_index: 0,
                    text: "Chunk 1 to append".to_string(),
                    suggested_title: "Chunk 1".to_string(),
                    candidates: vec![],
                    recommendation: LlmRecommendation {
                        action: "append".to_string(),
                        target_note_id: Some(target_id.0),
                        new_note_title: None,
                        parent_note_id: None,
                        reason: "fits math".to_string(),
                    },
                },
                ChunkDistributionPlan {
                    chunk_index: 1,
                    text: "Chunk 2 to create child".to_string(),
                    suggested_title: "Subtopic".to_string(),
                    candidates: vec![],
                    recommendation: LlmRecommendation {
                        action: "create_child".to_string(),
                        target_note_id: None,
                        new_note_title: Some("Calculus".to_string()),
                        parent_note_id: Some(target_id.0),
                        reason: "new topic under math".to_string(),
                    },
                },
            ],
        };

        // 4. Apply the distribution plan
        apply_distribution(&mut conn, plan).unwrap();

        // 5. Verify results
        let draft_note = crate::db::repos::notes::get_by_id(&conn, draft_id.0).unwrap().unwrap();
        assert_eq!(draft_note.is_draft, false);
        assert_eq!(draft_note.is_archived, true);

        let target_note = crate::db::repos::notes::get_by_id(&conn, target_id.0).unwrap().unwrap();
        assert!(target_note.content.starts_with("Initial math content\n\n<!-- imported from draft #"));
        assert!(target_note.content.ends_with("Chunk 1 to append"));
        assert_eq!(target_note.indexing_status, crate::domain::note::IndexingStatus::Stale);

        let mut stmt = conn.prepare("SELECT note_id, title, content, parent_note_id, is_draft FROM notes WHERE parent_note_id = ?").unwrap();
        let mut rows = stmt.query([target_id.0]).unwrap();
        let row = rows.next().unwrap().unwrap();
        let child_title: String = row.get(1).unwrap();
        let child_content: String = row.get(2).unwrap();
        let child_parent_id: i64 = row.get(3).unwrap();
        let child_is_draft: i64 = row.get(4).unwrap();

        assert_eq!(child_title, "Calculus");
        assert_eq!(child_content, "Chunk 2 to create child");
        assert_eq!(child_parent_id, target_id.0);
        assert_eq!(child_is_draft, 0);
    }
}
