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
                "INSERT INTO chunks (chunk_id, note_id, level, order_index, raw_text, raw_text_hash, clean_text, clean_text_hash, kind) VALUES (1, ?, 0, 0, 'Section 1 text', 'hash1', 'Section 1 text', 'hash1', 'heading_block')",
                [note_id.0],
            ).unwrap();
            conn.execute(
                "INSERT INTO cards (card_id, chunk_id, card_type, note_id) VALUES (1, 1, 'heading', ?)",
                [note_id.0],
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
                "INSERT INTO chunks (note_id, level, order_index, raw_text, raw_text_hash, clean_text, clean_text_hash, embedding, kind) VALUES (?, 1, 0, 'Line one of text', 'hash1', 'Line one of text', 'hash1', X'00', 'fragment')",
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
                "INSERT INTO chunks (note_id, level, order_index, raw_text, raw_text_hash, clean_text, clean_text_hash, token_count, embedding, kind) VALUES (?, 1, 0, 'This is a note about neural networks and machine learning.', 'hash1', 'This is a note about neural networks and machine learning.', 'hash1', 10, X'0000803f', 'fragment')",
                [nid.0],
            ).unwrap();
            conn.execute(
                "INSERT INTO chunks (note_id, level, order_index, raw_text, raw_text_hash, clean_text, clean_text_hash, token_count, embedding, kind) VALUES (?, 1, 1, 'Obsidian is a great tool for personal knowledge management.', 'hash2', 'Obsidian is a great tool for personal knowledge management.', 'hash2', 9, X'0000803f', 'fragment')",
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
            target_level: None,
        };

        let query_res = retrieve(&db_state, "knowledge management", None, &config, &options).await.unwrap();
        println!("query_res = {:#?}", query_res);
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
                    note_id,
                    section_id: sec.id,
                    card_type: scribo_lib::domain::card::CardType::Heading,
                    custom_front: None,
                    custom_back: None,
                    cloze_mask: None,
                    generated_by: Some("test_manual".to_string()),
                    source_raw_hash_at_creation: Some(sec.raw_hash.clone()),
                };
                scribo_lib::db::repos::cards::insert_with_schedule(conn, new_card).unwrap();
            }

            // Verify cards were created
            let cards = scribo_lib::db::repos::cards::list_by_note(conn, note_id.0).unwrap();
            assert_eq!(cards.len(), 2);
            assert_eq!(cards[0].card_type, scribo_lib::domain::CardType::Heading);
            assert_eq!(cards[0].status, scribo_lib::domain::card::CardLifecycle::Fresh);

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
            assert_eq!(cards_after[0].status, scribo_lib::domain::card::CardLifecycle::Fresh);

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
            assert_eq!(cards_v2[0].status, scribo_lib::domain::card::CardLifecycle::Fresh);
            assert_eq!(cards_v2[1].status, scribo_lib::domain::card::CardLifecycle::Stale);

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
                        note_id: sec.note_id,
                        section_id: sec.id,
                        card_type: scribo_lib::domain::card::CardType::Heading,
                        custom_front: None,
                        custom_back: None,
                        cloze_mask: None,
                        generated_by: Some("test_manual".to_string()),
                        source_raw_hash_at_creation: Some(sec.raw_hash),
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
            let mut stmt = conn.prepare("SELECT chunk_id, note_id, clean_text, embedding FROM chunks WHERE level = 1")?;
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
            let mut stmt = conn.prepare("SELECT card_id, chunk_id FROM cards")?;
            let mut rows = stmt.query([])?;
            while let Some(row) = rows.next().unwrap() {
                let card_id: i64 = row.get(0).unwrap();
                let section_id: i64 = row.get(1).unwrap();

                let card = scribo_lib::db::repos::cards::find_by_id(conn, scribo_lib::domain::CardId(card_id)).unwrap().unwrap();
                let section = scribo_lib::db::repos::sections::find_by_id(conn, scribo_lib::domain::SectionId(section_id)).unwrap().unwrap();
                let note = scribo_lib::db::repos::notes::get_by_id(conn, section.note_id.0).unwrap().unwrap();

                let rendered = card.render(Some(&section), note.id, note.title, note.path_cached);
                // Back should be exactly section.text_raw, which preserves raw markdown
                assert_eq!(rendered.back, section.text_raw, "Card back must preserve raw markdown from section");
            }

            Ok(())
        }).unwrap();

        // Cleanup
        let _ = std::fs::remove_file(db_path);
    }

    #[test]
    fn test_hierarchical_due_counts() {
        let db_state = setup_test_db();
        db_state.with_write(|conn| {
            // Create hierarchical notes structure:
            // Math
            //   Linear Algebra
            //   Calculus
            //     Integration
            let math_id = scribo_lib::db::repos::notes::insert(conn, &scribo_lib::domain::NewNote {
                title: "Math".to_string(),
                content: "Math root".to_string(),
                ..Default::default()
            }).unwrap();

            let la_id = scribo_lib::db::repos::notes::insert(conn, &scribo_lib::domain::NewNote {
                title: "Linear Algebra".to_string(),
                content: "LA content".to_string(),
                parent_note_id: Some(math_id),
                ..Default::default()
            }).unwrap();

            let calc_id = scribo_lib::db::repos::notes::insert(conn, &scribo_lib::domain::NewNote {
                title: "Calculus".to_string(),
                content: "Calc content".to_string(),
                parent_note_id: Some(math_id),
                ..Default::default()
            }).unwrap();

            let integration_id = scribo_lib::db::repos::notes::insert(conn, &scribo_lib::domain::NewNote {
                title: "Integration".to_string(),
                content: "Integration content".to_string(),
                parent_note_id: Some(calc_id),
                ..Default::default()
            }).unwrap();

            // Setup sections and cards for the notes
            // Section for Math
            let math_sec = conn.query_row(
                "INSERT INTO chunks (note_id, level, order_index, raw_text, raw_text_hash, clean_text, clean_text_hash, kind) VALUES (?, 0, 0, 'Math root', 'hash1', 'Math root', 'hash1', 'heading_block') RETURNING chunk_id",
                [math_id.0],
                |r| r.get::<_, i64>(0)
            ).unwrap();
            let card1 = conn.query_row(
                "INSERT INTO cards (chunk_id, note_id) VALUES (?, ?) RETURNING card_id",
                [math_sec, math_id.0],
                |r| r.get::<_, i64>(0)
            ).unwrap();

            // Section for Linear Algebra
            let la_sec = conn.query_row(
                "INSERT INTO chunks (note_id, level, order_index, raw_text, raw_text_hash, clean_text, clean_text_hash, kind) VALUES (?, 0, 0, 'LA content', 'hash2', 'LA content', 'hash2', 'heading_block') RETURNING chunk_id",
                [la_id.0],
                |r| r.get::<_, i64>(0)
            ).unwrap();
            let card2 = conn.query_row(
                "INSERT INTO cards (chunk_id, note_id) VALUES (?, ?) RETURNING card_id",
                [la_sec, la_id.0],
                |r| r.get::<_, i64>(0)
            ).unwrap();

            // Section for Integration
            let integration_sec = conn.query_row(
                "INSERT INTO chunks (note_id, level, order_index, raw_text, raw_text_hash, clean_text, clean_text_hash, kind) VALUES (?, 0, 0, 'Integration content', 'hash3', 'Integration content', 'hash3', 'heading_block') RETURNING chunk_id",
                [integration_id.0],
                |r| r.get::<_, i64>(0)
            ).unwrap();
            let card3 = conn.query_row(
                "INSERT INTO cards (chunk_id, note_id) VALUES (?, ?) RETURNING card_id",
                [integration_sec, integration_id.0],
                |r| r.get::<_, i64>(0)
            ).unwrap();

            // Create schedules for reviews
            let now = chrono::Utc::now().timestamp();
            // Note review for Calculus (due)
            conn.execute(
                "INSERT INTO schedules (target_type, target_id, next_review) VALUES ('note', ?, ?)",
                rusqlite::params![calc_id.0, now - 100]
            ).unwrap();

            // Card review for Math card1 (due)
            conn.execute(
                "INSERT INTO schedules (target_type, target_id, next_review) VALUES ('card', ?, ?)",
                rusqlite::params![card1, now - 50]
            ).unwrap();

            // Card review for Linear Algebra card2 (due)
            conn.execute(
                "INSERT INTO schedules (target_type, target_id, next_review) VALUES ('card', ?, ?)",
                rusqlite::params![card2, now - 10]
            ).unwrap();

            // Card review for Integration card3 (not due yet - in future)
            conn.execute(
                "INSERT INTO schedules (target_type, target_id, next_review) VALUES ('card', ?, ?)",
                rusqlite::params![card3, now + 1000]
            ).unwrap();

            // Query due counts
            let counts = scribo_lib::db::repos::schedules::get_hierarchical_due_counts(conn, now).unwrap();
            
            // Expected counts:
            // Math: card1 (1) + card2 (1) + calc_id note review (1) = 3
            // Linear Algebra: card2 (1) = 1
            // Calculus: calc_id note review (1) = 1 (Integration card3 is in future)
            // Integration: 0
            
            let math_count = counts.iter().find(|c| c.note_id == math_id.0).map(|c| c.due_count).unwrap_or(0);
            let la_count = counts.iter().find(|c| c.note_id == la_id.0).map(|c| c.due_count).unwrap_or(0);
            let calc_count = counts.iter().find(|c| c.note_id == calc_id.0).map(|c| c.due_count).unwrap_or(0);
            let integration_count = counts.iter().find(|c| c.note_id == integration_id.0).map(|c| c.due_count).unwrap_or(0);

            assert_eq!(math_count, 3);
            assert_eq!(la_count, 1);
            assert_eq!(calc_count, 1);
            assert_eq!(integration_count, 0);

            Ok(())
        }).unwrap();
    }

    #[test]
    fn test_get_repeat_mode_tree() {
        let db_state = setup_test_db();
        db_state.with_write(|conn| {
            // Create hierarchical notes structure:
            // Math (path_cached = "Math")
            //   Linear Algebra (path_cached = "Math/Linear Algebra")
            //   Calculus (path_cached = "Math/Calculus")
            // Economics (path_cached = "Economics")
            let math_id = scribo_lib::db::repos::notes::insert(conn, &scribo_lib::domain::NewNote {
                title: "Math".to_string(),
                content: "Math root".to_string(),
                parent_note_id: None,
                path_cached: Some("Math".to_string()),
                ..Default::default()
            }).unwrap();

            let la_id = scribo_lib::db::repos::notes::insert(conn, &scribo_lib::domain::NewNote {
                title: "Linear Algebra".to_string(),
                content: "LA content".to_string(),
                parent_note_id: Some(math_id),
                path_cached: Some("Math/Linear Algebra".to_string()),
                ..Default::default()
            }).unwrap();

            let calc_id = scribo_lib::db::repos::notes::insert(conn, &scribo_lib::domain::NewNote {
                title: "Calculus".to_string(),
                content: "Calc content".to_string(),
                parent_note_id: Some(math_id),
                path_cached: Some("Math/Calculus".to_string()),
                ..Default::default()
            }).unwrap();

            let economics_id = scribo_lib::db::repos::notes::insert(conn, &scribo_lib::domain::NewNote {
                title: "Economics".to_string(),
                content: "Economics root".to_string(),
                parent_note_id: None,
                path_cached: Some("Economics".to_string()),
                ..Default::default()
            }).unwrap();

            // Setup section and card for Linear Algebra
            let la_sec = conn.query_row(
                "INSERT INTO chunks (note_id, level, order_index, raw_text, raw_text_hash, clean_text, clean_text_hash, kind) VALUES (?, 0, 0, 'LA content', 'hash1', 'LA content', 'hash1', 'heading_block') RETURNING chunk_id",
                [la_id.0],
                |r| r.get::<_, i64>(0)
            ).unwrap();
            let card_la = conn.query_row(
                "INSERT INTO cards (chunk_id, note_id) VALUES (?, ?) RETURNING card_id",
                [la_sec, la_id.0],
                |r| r.get::<_, i64>(0)
            ).unwrap();

            // Setup section and cards for Calculus
            let calc_sec = conn.query_row(
                "INSERT INTO chunks (note_id, level, order_index, raw_text, raw_text_hash, clean_text, clean_text_hash, kind) VALUES (?, 0, 0, 'Calc content', 'hash2', 'Calc content', 'hash2', 'heading_block') RETURNING chunk_id",
                [calc_id.0],
                |r| r.get::<_, i64>(0)
            ).unwrap();
            let card_calc1 = conn.query_row(
                "INSERT INTO cards (chunk_id, note_id) VALUES (?, ?) RETURNING card_id",
                [calc_sec, calc_id.0],
                |r| r.get::<_, i64>(0)
            ).unwrap();
            let card_calc2 = conn.query_row(
                "INSERT INTO cards (chunk_id, note_id) VALUES (?, ?) RETURNING card_id",
                [calc_sec, calc_id.0],
                |r| r.get::<_, i64>(0)
            ).unwrap();
            let card_calc3 = conn.query_row(
                "INSERT INTO cards (chunk_id, note_id) VALUES (?, ?) RETURNING card_id",
                [calc_sec, calc_id.0],
                |r| r.get::<_, i64>(0)
            ).unwrap();

            // Create schedules for reviews
            let now = chrono::Utc::now().timestamp();
            
            // card_la: due
            conn.execute(
                "INSERT INTO schedules (target_type, target_id, next_review, state) VALUES ('card', ?, ?, 'review')",
                rusqlite::params![card_la, now - 100]
            ).unwrap();

            // card_calc1: due
            conn.execute(
                "INSERT INTO schedules (target_type, target_id, next_review, state) VALUES ('card', ?, ?, 'review')",
                rusqlite::params![card_calc1, now - 50]
            ).unwrap();

            // card_calc2: new (no schedule or state = new)
            conn.execute(
                "INSERT INTO schedules (target_type, target_id, next_review, state) VALUES ('card', ?, NULL, 'new')",
                rusqlite::params![card_calc2]
            ).unwrap();

            // card_calc3: review in future
            conn.execute(
                "INSERT INTO schedules (target_type, target_id, next_review, state) VALUES ('card', ?, ?, 'review')",
                rusqlite::params![card_calc3, now + 1000]
            ).unwrap();

            // Query repeat mode tree
            let nodes = scribo_lib::db::repos::schedules::get_repeat_mode_tree(conn, now).unwrap();
            
            // Expected counts:
            // Math: own_due=0, own_total=0, subtree_due=2 (la due + calc1 due), subtree_total=4 (la + calc1 + calc2 + calc3)
            // Linear Algebra: own_due=1, own_total=1, subtree_due=1, subtree_total=1
            // Calculus: own_due=1, own_total=3, subtree_due=1, subtree_total=3
            // Economics: own_due=0, own_total=0, subtree_due=0, subtree_total=0
            
            let math = nodes.iter().find(|n| n.note_id == math_id.0).unwrap();
            assert_eq!(math.own_due, 0);
            assert_eq!(math.own_total, 0);
            assert_eq!(math.subtree_due, 2);
            assert_eq!(math.subtree_total, 4);

            let la = nodes.iter().find(|n| n.note_id == la_id.0).unwrap();
            assert_eq!(la.own_due, 1);
            assert_eq!(la.own_total, 1);
            assert_eq!(la.subtree_due, 1);
            assert_eq!(la.subtree_total, 1);

            let calc = nodes.iter().find(|n| n.note_id == calc_id.0).unwrap();
            assert_eq!(calc.own_due, 1);
            assert_eq!(calc.own_total, 3);
            assert_eq!(calc.subtree_due, 1);
            assert_eq!(calc.subtree_total, 3);

            let eco = nodes.iter().find(|n| n.note_id == economics_id.0).unwrap();
            assert_eq!(eco.own_due, 0);
            assert_eq!(eco.own_total, 0);
            assert_eq!(eco.subtree_due, 0);
            assert_eq!(eco.subtree_total, 0);

            Ok(())
        }).unwrap();
    }

    #[test]
    fn test_hierarchical_tags_system() {
        use scribo_lib::db::repos::tags::{
            parse_and_resolve_tags, get_by_path, get_by_id, autocomplete_tags,
            associate_note_tag, get_note_tags, get_note_ids_by_tag, move_tag,
            rename_tag, delete_tag
        };
        use scribo_lib::domain::tag::TagSource;

        let db_state = setup_test_db();
        db_state.with_conn(|conn| {
            // 1. Parse and resolve tag paths
            let tags = parse_and_resolve_tags(conn, "#Chemistry/Microscope/Atom #important").unwrap();
            assert_eq!(tags.len(), 2);
            
            // Try tag without # prefix
            let tags_no_hash = parse_and_resolve_tags(conn, "Chemistry/Microscope/Atom").unwrap();
            assert_eq!(tags_no_hash.len(), 1);
            assert_eq!(tags_no_hash[0], tags[0]);
            
            // Check resolved leaf tags
            let atom_tag = get_by_path(conn, "chemistry/microscope/atom").unwrap().unwrap();
            let important_tag = get_by_path(conn, "important").unwrap().unwrap();
            assert_eq!(atom_tag.depth, 2);
            assert_eq!(important_tag.depth, 0);
            
            // 2. Test Autocomplete
            let autocomplete = autocomplete_tags(conn, "chem", 10).unwrap();
            assert_eq!(autocomplete.len(), 3); // chemistry, chemistry/microscope, chemistry/microscope/atom
            assert_eq!(autocomplete[0].name, "Chemistry");
            assert_eq!(autocomplete[1].name, "Microscope");
            
            // 3. Test note tag association
            let note = scribo_lib::domain::note::NewNote {
                title: "Atomic Structure".to_string(),
                content: "Electrons and protons.".to_string(),
                ..Default::default()
            };
            let note_id = scribo_lib::db::repos::notes::insert(conn, &note).unwrap();
            associate_note_tag(conn, note_id, atom_tag.tag_id, TagSource::Manual, None).unwrap();
            
            let note_tags = get_note_tags(conn, note_id).unwrap();
            assert_eq!(note_tags.len(), 1);
            assert_eq!(note_tags[0].name, "Atom");
            
            // 4. Test closure query for notes (Chemistry subtree)
            let note_ids = get_note_ids_by_tag(conn, "chemistry", true).unwrap();
            assert_eq!(note_ids.len(), 1);
            assert_eq!(note_ids[0], note_id.0);
            
            // Without subtree, should be empty (since note is associated with Atom leaf)
            let note_ids_flat = get_note_ids_by_tag(conn, "chemistry", false).unwrap();
            assert!(note_ids_flat.is_empty());
            
            // 5. Test move tag (Atom moves to important)
            move_tag(conn, atom_tag.tag_id, Some(important_tag.tag_id)).unwrap();
            let updated_atom = get_by_id(conn, atom_tag.tag_id).unwrap().unwrap();
            assert_eq!(updated_atom.depth, 1);
            assert_eq!(updated_atom.path_cached, "important/atom");
            
            // 6. Test move cycle check
            let cycle_err = move_tag(conn, important_tag.tag_id, Some(atom_tag.tag_id));
            assert!(cycle_err.is_err());
            
            // 7. Test rename tag
            rename_tag(conn, important_tag.tag_id, "Highly-Important").unwrap();
            let updated_important = get_by_id(conn, important_tag.tag_id).unwrap().unwrap();
            assert_eq!(updated_important.path_cached, "highly-important");
            
            // Verify descendants' path updated
            let updated_atom_after_rename = get_by_id(conn, atom_tag.tag_id).unwrap().unwrap();
            assert_eq!(updated_atom_after_rename.path_cached, "highly-important/atom");
            
            // 8. Test delete tag (ON DELETE CASCADE)
            delete_tag(conn, important_tag.tag_id).unwrap();
            assert!(get_by_id(conn, important_tag.tag_id).unwrap().is_none());
            assert!(get_by_id(conn, atom_tag.tag_id).unwrap().is_none()); // cascaded
            
            // note_tags also cascaded
            let note_tags_after_delete = get_note_tags(conn, note_id).unwrap();
            assert!(note_tags_after_delete.is_empty());
            
            Ok(())
        }).unwrap();
    }
}

