#[cfg(test)]
mod tests {
    use crate::domain::note::NewNote;
    use super::super::{split_into_topics, DraftDistributionPlan, ChunkDistributionPlan, LlmRecommendation, apply_distribution};

    #[test]
    fn test_split_into_topics() {
        let content = "\
# Math Note
This is some content.

## Section 2
And some more content here.
- Item 1
- Item 2
";
        let chunks = split_into_topics(content, 1000);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].suggested_title, "Math Note");
        assert!(chunks[0].text.contains("This is some content."));
        assert_eq!(chunks[1].suggested_title, "Section 2");
        assert!(chunks[1].text.contains("Item 2"));
    }

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
