#[cfg(test)]
mod tests {
    use scribo_lib::db::schema::initialize_schema;
    use scribo_lib::db::repos::cards::{review_fsrs, CardReviewParams};
    use scribo_lib::db::repos::files::update_content_with_diff;
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
        DbState {
            pool: RwLock::new(Some(pool)),
            write_lock: Mutex::new(()),
        }
    }

    #[test]
    fn test_fsrs_card_review_lifecycle() {
        let db_state = setup_test_db();

        db_state.with_conn(|conn| {
            conn.execute(
                "INSERT INTO files (file_id, file_path, file_name, status) VALUES (1, 'note.md', 'note.md', 'indexed')",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO cards (card_id, file_id, state) VALUES (1, 1, 'new')",
                [],
            ).unwrap();
            Ok(())
        }).unwrap();

        let params = CardReviewParams {
            card_id: 1,
            rating: 3,
        };

        db_state.with_conn(|conn| {
            let result = review_fsrs(conn, params).unwrap();
            assert!(result.scheduled_days > 0.0);
            assert!(result.next_review > 0);
            Ok(())
        }).unwrap();

        db_state.with_conn(|conn| {
            let state: String = conn.query_row("SELECT state FROM cards WHERE card_id = 1", [], |r| r.get(0)).unwrap();
            assert_eq!(state, "learning");

            let log_count: i64 = conn.query_row("SELECT COUNT(*) FROM review_logs WHERE card_id = 1", [], |r| r.get(0)).unwrap();
            assert_eq!(log_count, 1);
            Ok(())
        }).unwrap();
    }

    #[test]
    fn test_version_control_diffy() {
        let db_state = setup_test_db();

        db_state.with_conn(|conn| {
            conn.execute("INSERT INTO files (file_id, file_path, file_name) VALUES (1, 'a.md', 'a.md')", []).unwrap();
            conn.execute(
                "INSERT INTO chunks (file_id, chunk_index, chunk_text, embedding) VALUES (1, 0, 'Line one of text', X'00')",
                [],
            ).unwrap();
            Ok(())
        }).unwrap();

        let new_text = "Line one of text\nLine two is added".to_string();
        db_state.with_conn(|conn| {
            update_content_with_diff(conn, 1, &new_text).unwrap();
            Ok(())
        }).unwrap();

        db_state.with_conn(|conn| {
            let patch: String = conn.query_row("SELECT patch FROM files_history WHERE file_id = 1", [], |r| r.get(0)).unwrap();
            assert!(patch.contains("+Line two is added"));
            Ok(())
        }).unwrap();
    }

    #[tokio::test]
    async fn test_retrieval_pipeline_and_fetch() {
        let db_state = setup_test_db();

        db_state.with_conn(|conn| {
            conn.execute("INSERT INTO files (file_id, file_path, file_name) VALUES (1, 'doc.md', 'doc.md')", []).unwrap();
            conn.execute(
                "INSERT INTO chunks (file_id, chunk_index, chunk_text, token_count, embedding) VALUES (1, 0, 'This is a note about neural networks and machine learning.', 10, X'0000803f')",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO chunks (file_id, chunk_index, chunk_text, token_count, embedding) VALUES (1, 1, 'Obsidian is a great tool for personal knowledge management.', 9, X'0000803f')",
                [],
            ).unwrap();
            Ok(())
        }).unwrap();

        // 1. Test Fetch
        let fetch_query = FetchQuery {
            file_path: Some("doc.md".to_string()),
            file_name: None,
            include_deleted: Some(false),
            limit: Some(2),
            offset: Some(0),
        };
        let fetch_res = fetch(&db_state, &fetch_query).unwrap();
        assert_eq!(fetch_res.len(), 2);
        assert_eq!(fetch_res[0].file_path, "doc.md");
        assert_eq!(fetch_res[0].chunk_index, 0);

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
        assert_eq!(query_res[0].chunk_ref.file_path, "doc.md");
        assert_eq!(query_res[0].chunk_ref.chunk_index, 1);
    }
}
