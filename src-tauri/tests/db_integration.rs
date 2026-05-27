#[cfg(test)]
mod tests {
    use scribo_lib::db::schema::initialize_schema;
    use scribo_lib::db::repos::notes::update_content_with_diff;
    use scribo_lib::retrieval::{retrieve, fetch, RetrievalConfig, RetrieveOptions, FetchQuery};
    use scribo_lib::DbState;
    use parking_lot::{Mutex, RwLock};
    use r2d2::Pool;
    use r2d2_sqlite::SqliteConnectionManager;

    fn setup_test_db() -> DbState {
        let manager = SqliteConnectionManager::memory();
        let pool = Pool::builder().max_size(1).build(manager).unwrap();
        {
            let mut conn = pool.get().unwrap();
            initialize_schema(&mut conn).unwrap();
        }
        let pool_arc = std::sync::Arc::new(RwLock::new(Some(pool)));
        let reviewer = std::sync::Arc::new(scribo_lib::services::reviewer::ReviewerService::new());
        DbState {
            pool: pool_arc,
            reviewer,
            write_lock: Mutex::new(()),
        }
    }

    #[test]
    fn test_fsrs_card_review_lifecycle() {
        let db_state = setup_test_db();

        db_state.with_conn(|conn| {
            let note = scribo_lib::domain::note::NewNote {
                title: "note.md".to_string(),
                content: "content".to_string(),
                ..Default::default()
            };
            let note_id = scribo_lib::db::repos::notes::insert(conn, &note).unwrap();

            conn.execute(
                "INSERT INTO sections (section_id, note_id, section_index, text_raw, source_hash) VALUES (1, ?, 0, 'Section 1 text', 'hash1')",
                [note_id.0],
            ).unwrap();
            conn.execute(
                "INSERT INTO cards (card_id, section_id, card_type) VALUES (1, 1, 'heading')",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO schedules (target_type, target_id, state) VALUES ('card', 1, 'new')",
                [],
            ).unwrap();
            Ok(())
        }).unwrap();

        db_state.with_write(|conn| {
            let result = db_state.reviewer.rate_review(
                conn,
                scribo_lib::domain::ScheduleId(1),
                scribo_lib::domain::Rating::Good,
            ).unwrap();
            assert!(result.scheduled_days > 0.0);
            assert!(result.next_review > 0);
            Ok(())
        }).unwrap();

        db_state.with_conn(|conn| {
            let state: String = conn.query_row("SELECT state FROM schedules WHERE target_type = 'card' AND target_id = 1", [], |r| r.get(0)).unwrap();
            assert_eq!(state, "learning");

            let log_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM review_logs rl
                 JOIN schedules s ON rl.schedule_id = s.schedule_id
                 WHERE s.target_type = 'card' AND s.target_id = 1",
                [],
                |r| r.get(0)
            ).unwrap();
            assert_eq!(log_count, 1);
            Ok(())
        }).unwrap();
    }

    #[test]
    fn test_version_control_diffy() {
        let db_state = setup_test_db();

        let note_id = db_state.with_conn(|conn| {
            let note = scribo_lib::domain::note::NewNote {
                title: "a.md".to_string(),
                content: "Line one of text".to_string(),
                ..Default::default()
            };
            let nid = scribo_lib::db::repos::notes::insert(conn, &note).unwrap();
            conn.execute(
                "INSERT INTO fragments (note_id, fragment_index, text_clean, source_hash, embedding) VALUES (?, 0, 'Line one of text', 'hash1', X'00')",
                [nid.0],
            ).unwrap();
            Ok(nid.0)
        }).unwrap();

        let new_text = "Line one of text\nLine two is added".to_string();
        db_state.with_conn(|conn| {
            update_content_with_diff(conn, note_id, &new_text).unwrap();
            Ok(())
        }).unwrap();

        db_state.with_conn(|conn| {
            let patch: String = conn.query_row("SELECT patch FROM note_revisions WHERE note_id = ?", [note_id], |r| r.get(0)).unwrap();
            assert!(patch.contains("+Line two is added"));
            Ok(())
        }).unwrap();
    }

    #[tokio::test]
    async fn test_retrieval_pipeline_and_fetch() {
        let db_state = setup_test_db();

        let note_id = db_state.with_conn(|conn| {
            let note = scribo_lib::domain::note::NewNote {
                title: "doc.md".to_string(),
                content: "This is a note about neural networks and machine learning.\nObsidian is a great tool for personal knowledge management.".to_string(),
                ..Default::default()
            };
            let nid = scribo_lib::db::repos::notes::insert(conn, &note).unwrap();
            conn.execute(
                "INSERT INTO fragments (note_id, fragment_index, text_clean, source_hash, token_count, embedding) VALUES (?, 0, 'This is a note about neural networks and machine learning.', 'hash1', 10, X'0000803f')",
                [nid.0],
            ).unwrap();
            conn.execute(
                "INSERT INTO fragments (note_id, fragment_index, text_clean, source_hash, token_count, embedding) VALUES (?, 1, 'Obsidian is a great tool for personal knowledge management.', 'hash2', 9, X'0000803f')",
                [nid.0],
            ).unwrap();
            Ok(nid.0)
        }).unwrap();

        // 1. Test Fetch
        let fetch_query = FetchQuery {
            note_id: Some(scribo_lib::domain::note::NoteId(note_id)),
            include_deleted: Some(false),
            limit: Some(2),
            offset: Some(0),
        };
        let fetch_res = fetch(&db_state, &fetch_query).unwrap();
        assert_eq!(fetch_res.len(), 2);
        assert_eq!(fetch_res[0].note_id.0, note_id);
        assert_eq!(fetch_res[0].fragment_index, 0);

        // 2. Test Retrieve (Keyword mode)
        let config = RetrievalConfig {
            mode: "keyword".to_string(),
            embedding_weight: None,
            pipeline: None,
            ai_rerank: None,
            vault_lang: Some("en".to_string()),
            llm_config: None,
        };
        let options = RetrieveOptions {
            top_k: Some(1),
            filters: None,
        };

        let query_res = retrieve(&db_state, "knowledge management", None, &config, &options).await.unwrap();
        assert_eq!(query_res.len(), 1);
        assert_eq!(query_res[0].fragment_ref.note_id.0, note_id);
        assert_eq!(query_res[0].fragment_ref.fragment_index, 1);
    }

    #[test]
    fn test_idempotent_reindexing_and_stale_cards() {
        let db_state = setup_test_db();

        let title = "math.md".to_string();
        let content_v1 = "## Math\n\nSome algebra text.\n\n## Calculus\n\nSome integration text.".to_string();

        db_state.with_conn(|conn| {
            // First we need to insert the note record into the DB with 'pending' status
            let note = scribo_lib::domain::note::NewNote {
                title: title.clone(),
                content: content_v1.clone(),
                ..Default::default()
            };
            let note_id = scribo_lib::db::repos::notes::insert(conn, &note).unwrap();
            assert!(note_id.0 > 0);

            // Index the note for the first time
            let payload = scribo_lib::services::indexer::IndexingPayload {
                note_id: note_id.0,
                embedding_model: "test-model",
                embedding_dim: 1536,
                indexing_version: "1",
            };
            let persisted_id = scribo_lib::services::indexer::persist_indexed_file(conn, payload).unwrap();
            assert_eq!(persisted_id, note_id.0);

            // Verify sections were created
            let sections = scribo_lib::db::repos::sections::list_by_note(conn, note_id.0).unwrap();
            assert_eq!(sections.len(), 2);
            assert_eq!(sections[0].heading.as_deref(), Some("Math"));
            assert_eq!(sections[1].heading.as_deref(), Some("Calculus"));

            // Manually create cards for the sections (since indexing no longer auto-creates them)
            for sec in &sections {
                let new_card = scribo_lib::domain::card::NewCard {
                    section_id: sec.id,
                    card_type: scribo_lib::domain::card::CardType::Heading,
                    custom_front: None,
                    custom_back: None,
                    cloze_mask: None,
                    generated_by: Some("test_manual".to_string()),
                    section_hash_at_creation: Some(sec.source_hash.clone()),
                };
                scribo_lib::db::repos::cards::insert_with_schedule(conn, new_card).unwrap();
            }

            // Verify cards were created
            let cards = scribo_lib::db::repos::cards::list_by_note(conn, note_id.0).unwrap();
            assert_eq!(cards.len(), 2);
            assert_eq!(cards[0].card_type, scribo_lib::domain::CardType::Heading);
            assert_eq!(cards[0].is_stale, false);

            // Re-index with same content: check that it is idempotent and no cards are marked stale
            let payload_same = scribo_lib::services::indexer::IndexingPayload {
                note_id: note_id.0,
                embedding_model: "test-model",
                embedding_dim: 1536,
                indexing_version: "1",
            };
            scribo_lib::services::indexer::persist_indexed_file(conn, payload_same).unwrap();
            let cards_after = scribo_lib::db::repos::cards::list_by_note(conn, note_id.0).unwrap();
            assert_eq!(cards_after.len(), 2);
            assert_eq!(cards_after[0].is_stale, false);

            // Re-index with modified content: update the content first, then change only the second section
            let content_v2 = "## Math\n\nSome algebra text.\n\n## Calculus\n\nSome advanced integration text.".to_string();
            scribo_lib::db::repos::notes::set_content(conn, note_id.0, &content_v2).unwrap();

            let payload_v2 = scribo_lib::services::indexer::IndexingPayload {
                note_id: note_id.0,
                embedding_model: "test-model",
                embedding_dim: 1536,
                indexing_version: "1",
            };
            scribo_lib::services::indexer::persist_indexed_file(conn, payload_v2).unwrap();

            // The first card (Math) should NOT be stale, the second card (Calculus) should be stale!
            let cards_v2 = scribo_lib::db::repos::cards::list_by_note(conn, note_id.0).unwrap();
            assert_eq!(cards_v2.len(), 2);
            assert_eq!(cards_v2[0].is_stale, false);
            assert_eq!(cards_v2[1].is_stale, true);

            Ok(())
        }).unwrap();
    }

    #[tokio::test]
    async fn test_obsidian_import_real_embedding() {
        let import_dir = "/home/yoffens/obsidian2026/1-INBOX/";
        if !std::path::Path::new(import_dir).exists() {
            println!("WARNING: Target directory {} does not exist. Skipping real obsidian import test.", import_dir);
            return;
        }

        // 1. Setup real file database for maximum parity
        let db_path = "target/test_import_real.db";
        let _ = std::fs::remove_file(db_path); // ensure clean state
        let manager = SqliteConnectionManager::file(db_path);
        let pool = Pool::builder().max_size(1).build(manager).unwrap();
        {
            let mut conn = pool.get().unwrap();
            initialize_schema(&mut conn).unwrap();
        }
        let pool_arc = std::sync::Arc::new(RwLock::new(Some(pool)));
        let reviewer = std::sync::Arc::new(scribo_lib::services::reviewer::ReviewerService::new());
        let db_state = DbState {
            pool: pool_arc,
            reviewer,
            write_lock: Mutex::new(()),
        };

        // 2. Initialize the local embedder with the user's granite model
        let embedder_config = scribo_lib::ai::types::EmbedderConfig {
            provider: "local".to_string(),
            model: Some("granite-embedding-97M-multilingual-r2-BF16".to_string()),
            api_key: None,
            base_url: None,
        };
        let embedder = scribo_lib::ai::embedding::Embedder::new(embedder_config);

        // 3. Scan directory recursively for markdown files
        let mut md_files = Vec::new();
        for entry in walkdir::WalkDir::new(import_dir)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() {
                if let Some(ext) = entry.path().extension() {
                    if ext == "md" {
                        md_files.push(entry.path().to_path_buf());
                    }
                }
            }
        }

        if md_files.is_empty() {
            println!("WARNING: No markdown files found in {}. Skipping test assertions.", import_dir);
            return;
        }

        println!("Found {} markdown files to import.", md_files.len());

        // 4. Import files
        for path in &md_files {
            let note_id = db_state.with_write(|conn| {
                scribo_lib::services::import::import_markdown_file(conn, path)
            }).unwrap();

            // 4a. Run indexer inside write lock
            db_state.with_write(|conn| {
                let payload = scribo_lib::services::indexer::IndexingPayload {
                    note_id: note_id.0,
                    embedding_model: "granite-embedding-97M-multilingual-r2-BF16",
                    embedding_dim: 384,
                    indexing_version: "1",
                };
                scribo_lib::services::indexer::persist_indexed_file(conn, payload)
            }).unwrap();

            // 4b. Fetch fragments that need embeddings
            let fragments = db_state.with_conn(|conn| {
                scribo_lib::db::repos::fragments::list_by_note(conn, note_id.0)
            }).unwrap();

            // 4c. Generate embeddings asynchronously
            let mut fragment_embeddings: Vec<(i64, Vec<f32>)> = Vec::new();
            for frag in fragments {
                let emb = embedder.embed(&frag.text_clean).await.unwrap();
                assert_eq!(emb.len(), 384, "Embedding dimension should be 384");
                fragment_embeddings.push((frag.fragment_index, emb));
            }

            // 4d. Save embeddings & manually create cards for each section inside write lock
            db_state.with_write(|conn| {
                for (index, emb) in fragment_embeddings {
                    let emb_bytes = bytemuck::cast_slice::<f32, u8>(&emb).to_vec();
                    scribo_lib::db::repos::fragments::set_embedding(conn, note_id.0, index, &emb_bytes)?;
                }

                let sections = scribo_lib::db::repos::sections::list_by_note(conn, note_id.0)?;
                for sec in sections {
                    let new_card = scribo_lib::domain::card::NewCard {
                        section_id: sec.id,
                        card_type: scribo_lib::domain::card::CardType::Heading,
                        custom_front: None,
                        custom_back: None,
                        cloze_mask: None,
                        generated_by: Some("test_manual".to_string()),
                        section_hash_at_creation: Some(sec.source_hash),
                    };
                    scribo_lib::db::repos::cards::insert_with_schedule(conn, new_card)?;
                }
                Ok(())
            }).unwrap();
        }

        // 5. Assertions on the final state
        db_state.with_conn(|conn| {
            // Count notes
            let note_count: i64 = conn.query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0)).unwrap();
            assert!(note_count > 0, "Should have successfully imported at least one note");

            // Verify fragments
            let mut stmt = conn.prepare("SELECT fragment_id, note_id, text_clean, embedding FROM fragments")?;
            let mut rows = stmt.query([])?;
            while let Some(row) = rows.next().unwrap() {
                let text_clean: String = row.get(2).unwrap();
                let embedding: Vec<u8> = row.get(3).unwrap();

                // Check text_clean is indeed clean (e.g. no Markdown headers prefix)
                assert!(!text_clean.is_empty(), "Clean fragment text should not be empty");
                assert!(!text_clean.starts_with("# "), "Clean fragment text should not start with header: {}", text_clean);
                assert!(!text_clean.starts_with("## "), "Clean fragment text should not start with header: {}", text_clean);
                assert!(!text_clean.starts_with("### "), "Clean fragment text should not start with header: {}", text_clean);

                // Check embedding matches expected size (384 * 4 bytes)
                assert_eq!(embedding.len(), 384 * 4, "Embedding blob should be 1536 bytes (384 floats)");
            }

            // Verify cards render raw markdown
            let mut stmt = conn.prepare("SELECT card_id, section_id FROM cards")?;
            let mut rows = stmt.query([])?;
            while let Some(row) = rows.next().unwrap() {
                let card_id: i64 = row.get(0).unwrap();
                let section_id: i64 = row.get(1).unwrap();

                let card = scribo_lib::db::repos::cards::find_by_id(conn, scribo_lib::domain::CardId(card_id)).unwrap().unwrap();
                let section = scribo_lib::db::repos::sections::find_by_id(conn, scribo_lib::domain::SectionId(section_id)).unwrap().unwrap();

                let rendered = card.render(&section);
                // Back should be exactly section.text_raw, which preserves raw markdown
                assert_eq!(rendered.back, section.text_raw, "Card back must preserve raw markdown from section");
            }

            Ok(())
        }).unwrap();

        // Cleanup
        let _ = std::fs::remove_file(db_path);
    }
}
