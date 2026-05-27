use rusqlite::Connection;
use crate::error::AppError;
use crate::domain::distribute::{DraftDistributionPlan, DistributeAction, ChunkDistributionPlan};
use crate::domain::note::{NoteId, NewNote, IndexingStatus, NoteLifecycle};

pub fn apply_distribution(
    conn: &mut Connection,
    plan: DraftDistributionPlan,
) -> Result<Vec<i64>, AppError> {
    let result_json = serde_json::to_string(&plan).unwrap_or_default();
    
    // 1. Resolve "merge_with_chunk" decisions first
    let mut chunks = plan.chunks.clone();
    resolve_merge_actions(&mut chunks);

    let mut affected_note_ids = Vec::new();

    // 2. Apply all 'append' actions
    apply_append_actions(conn, plan.draft_id, &chunks, &mut affected_note_ids)?;

    // 3. Insert new child notes (handling negative parent references and fallback ordering)
    apply_create_child_actions(conn, &chunks, &mut affected_note_ids)?;

    // 4. Archive the draft note if none of the chunks were skipped.
    let has_skipped = plan.chunks.iter().any(|c| matches!(c.recommendation.action, DistributeAction::Skip { .. }));
    if !has_skipped {
        crate::db::repos::notes::archive_note(conn, plan.draft_id)?;
    }

    // 5. Log the applied distribution run using pre-serialized result_json
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

    Ok(affected_note_ids)
}

/// Resolves chunk merges transitively by appending text to target chunks and skipping merged chunks.
fn resolve_merge_actions(chunks: &mut [ChunkDistributionPlan]) {
    let mut resolved_any = true;
    let mut iterations = 0;
    while resolved_any && iterations < 100 {
        resolved_any = false;
        iterations += 1;
        
        for i in 0..chunks.len() {
            if let DistributeAction::MergeWithChunk { chunk_index } = chunks[i].recommendation.action {
                let target_idx = chunk_index;
                if target_idx < chunks.len() && target_idx != i {
                    let text_to_append = chunks[i].text.clone();
                    if !text_to_append.is_empty() {
                        if chunks[target_idx].text.is_empty() {
                            chunks[target_idx].text = text_to_append;
                        } else {
                            chunks[target_idx].text.push_str("\n\n");
                            chunks[target_idx].text.push_str(&text_to_append);
                        }
                    }
                    chunks[i].recommendation.action = DistributeAction::Skip;
                    chunks[i].recommendation.reason = "Merged into another chunk".to_string();
                    chunks[i].text.clear();
                    resolved_any = true;
                }
            }
        }
    }
}

/// Processes all 'Append' recommendations.
fn apply_append_actions(
    conn: &mut Connection,
    draft_id: i64,
    chunks: &[ChunkDistributionPlan],
    affected_note_ids: &mut Vec<i64>,
) -> Result<(), AppError> {
    for chunk in chunks {
        if let DistributeAction::Append { target_note_id, .. } = &chunk.recommendation.action {
            let target_id = target_note_id.0;
            let target_note = crate::db::repos::notes::get_by_id(conn, target_id)?
                .ok_or_else(|| AppError::Other(format!("Target note not found: {}", target_id)))?;
            
            let separator = format!(
                "\n\n<!-- imported from draft #{} on {} -->\n",
                draft_id,
                chrono::Utc::now().to_rfc3339()
            );
            let new_content = if target_note.content.is_empty() {
                chunk.text.clone()
            } else {
                format!("{}{}{}", target_note.content, separator, chunk.text)
            };
            
            crate::db::repos::notes::update_content_with_diff(conn, target_id, &new_content)?;
            crate::db::repos::notes::set_status(conn, target_id, IndexingStatus::Pending)?;
            affected_note_ids.push(target_id);

            if let Some(tags) = &chunk.recommendation.tags {
                for tag_str in tags {
                    if let Ok(tag_ids) = crate::db::repos::tags::parse_and_resolve_tags(conn, tag_str) {
                        for tag_id in tag_ids {
                            let _ = crate::db::repos::tags::associate_note_tag(
                                conn,
                                NoteId(target_id),
                                tag_id,
                                crate::domain::tag::TagSource::Ai,
                                None,
                            );
                            let _ = crate::db::repos::tags::inherit_note_tags_to_chunks(conn, NoteId(target_id), tag_id);
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// Processes all 'CreateChild' recommendations.
fn apply_create_child_actions(
    conn: &mut Connection,
    chunks: &[ChunkDistributionPlan],
    affected_note_ids: &mut Vec<i64>,
) -> Result<(), AppError> {
    let mut inserted_notes = std::collections::HashMap::new();
    let mut pending_creations: Vec<ChunkDistributionPlan> = chunks.iter()
        .filter(|c| matches!(c.recommendation.action, DistributeAction::CreateChild { .. }))
        .cloned()
        .collect();

    let mut progressed = true;
    while !pending_creations.is_empty() && progressed {
        progressed = false;
        let mut next_pending = Vec::new();

        for chunk in pending_creations {
            let (parent_id_opt, new_note_title) = match &chunk.recommendation.action {
                DistributeAction::CreateChild { parent_note_id, new_note_title } => {
                    (parent_note_id.map(|id| id.0), new_note_title.clone())
                }
                _ => continue,
            };
            
            let can_insert = if let Some(pid) = parent_id_opt {
                if pid < 0 {
                    let parent_chunk_idx = (-pid - 1) as usize;
                    inserted_notes.contains_key(&parent_chunk_idx)
                } else {
                    true
                }
            } else {
                true
            };

            if can_insert {
                let title = if new_note_title.is_empty() {
                    chunk.suggested_title.clone()
                } else {
                    new_note_title
                };

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
                    content: chunk.text.clone(),
                    parent_note_id: parent_id,
                    lifecycle: Some(NoteLifecycle::Active),
                    ..Default::default()
                };
                let new_id = crate::db::repos::notes::insert(conn, &new_note)?;
                inserted_notes.insert(chunk.chunk_index, new_id.0);
                affected_note_ids.push(new_id.0);
                progressed = true;

                if let Some(tags) = &chunk.recommendation.tags {
                    for tag_str in tags {
                        if let Ok(tag_ids) = crate::db::repos::tags::parse_and_resolve_tags(conn, tag_str) {
                            for tag_id in tag_ids {
                                let _ = crate::db::repos::tags::associate_note_tag(
                                    conn,
                                    new_id,
                                    tag_id,
                                    crate::domain::tag::TagSource::Ai,
                                    None,
                                );
                                let _ = crate::db::repos::tags::inherit_note_tags_to_chunks(conn, new_id, tag_id);
                            }
                        }
                    }
                }
            } else {
                next_pending.push(chunk);
            }
        }

        pending_creations = next_pending;
    }

    // Safe fallback if circular dependency occurs
    for chunk in pending_creations {
        let new_note_title = match &chunk.recommendation.action {
            DistributeAction::CreateChild { new_note_title, .. } => new_note_title.clone(),
            _ => continue,
        };
        let title = if new_note_title.is_empty() {
            chunk.suggested_title.clone()
        } else {
            new_note_title
        };

        let new_note = NewNote {
            title,
            content: chunk.text.clone(),
            parent_note_id: None,
            lifecycle: Some(NoteLifecycle::Active),
            ..Default::default()
        };
        let new_id = crate::db::repos::notes::insert(conn, &new_note)?;
        affected_note_ids.push(new_id.0);

        if let Some(tags) = &chunk.recommendation.tags {
            for tag_str in tags {
                if let Ok(tag_ids) = crate::db::repos::tags::parse_and_resolve_tags(conn, tag_str) {
                    for tag_id in tag_ids {
                        let _ = crate::db::repos::tags::associate_note_tag(
                            conn,
                            new_id,
                            tag_id,
                            crate::domain::tag::TagSource::Ai,
                            None,
                        );
                        let _ = crate::db::repos::tags::inherit_note_tags_to_chunks(conn, new_id, tag_id);
                    }
                }
            }
        }
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
            lifecycle: Some(NoteLifecycle::Draft),
            ..Default::default()
        };
        let draft_id = crate::db::repos::notes::insert(&conn, &draft).unwrap();

        // 2. Insert a target note to append to
        let target = NewNote {
            title: "Math".to_string(),
            content: "Initial math content".to_string(),
            lifecycle: Some(NoteLifecycle::Active),
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
                        action: DistributeAction::Append {
                            target_note_id: target_id,
                            target_section_id: None,
                        },
                        tags: Some(vec!["#Math".to_string()]),
                        confidence: Some(0.9),
                        reason: "fits math".to_string(),
                    },
                },
                ChunkDistributionPlan {
                    chunk_index: 1,
                    text: "Chunk 2 to create child".to_string(),
                    suggested_title: "Subtopic".to_string(),
                    candidates: vec![],
                    recommendation: LlmRecommendation {
                        action: DistributeAction::CreateChild {
                            parent_note_id: Some(target_id),
                            new_note_title: "Calculus".to_string(),
                        },
                        tags: Some(vec!["#Math/Calculus".to_string()]),
                        confidence: Some(0.95),
                        reason: "new topic under math".to_string(),
                    },
                },
            ],
        };

        // 4. Apply the distribution plan
        apply_distribution(&mut conn, plan).unwrap();

        // 5. Verify results
        let draft_note = crate::db::repos::notes::get_by_id(&conn, draft_id.0).unwrap().unwrap();
        assert_eq!(draft_note.lifecycle, NoteLifecycle::Archived);

        let target_note = crate::db::repos::notes::get_by_id(&conn, target_id.0).unwrap().unwrap();
        assert!(target_note.content.starts_with("Initial math content\n\n<!-- imported from draft #"));
        assert!(target_note.content.ends_with("Chunk 1 to append"));
        assert_eq!(target_note.indexing_status, crate::domain::note::IndexingStatus::Pending);

        let mut stmt = conn.prepare("SELECT note_id, title, content, parent_note_id, lifecycle FROM notes WHERE parent_note_id = ?").unwrap();
        let mut rows = stmt.query([target_id.0]).unwrap();
        let row = rows.next().unwrap().unwrap();
        let child_title: String = row.get(1).unwrap();
        let child_content: String = row.get(2).unwrap();
        let child_parent_id: i64 = row.get(3).unwrap();
        let child_lifecycle_str: String = row.get(4).unwrap();

        assert_eq!(child_title, "Calculus");
        assert_eq!(child_content, "Chunk 2 to create child");
        assert_eq!(child_parent_id, target_id.0);
        assert_eq!(child_lifecycle_str, "active");

        // Verify tags
        let target_tags = crate::db::repos::tags::get_note_tags(&conn, target_id).unwrap();
        assert_eq!(target_tags.len(), 1);
        assert_eq!(target_tags[0].name, "Math");

        let child_note_id = row.get::<_, i64>(0).unwrap();
        let child_tags = crate::db::repos::tags::get_note_tags(&conn, NoteId(child_note_id)).unwrap();
        assert_eq!(child_tags.len(), 1);
        assert_eq!(child_tags[0].name, "Calculus");
        assert_eq!(child_tags[0].path_cached, "math/calculus");
    }
}
