use crate::commands::cards::{cards_review_fsrs_impl, CardReviewParams};
use crate::commands::files::files_update_content_with_diff_impl;
use crate::schema::initialize_schema;
use crate::DbState;
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

    let result = cards_review_fsrs_impl(&db_state, params).unwrap();

    assert!(result.scheduled_days > 0.0);
    assert!(result.next_review > 0);

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
    files_update_content_with_diff_impl(&db_state, 1, new_text).unwrap();

    db_state.with_conn(|conn| {
        let patch: String = conn.query_row("SELECT patch FROM files_history WHERE file_id = 1", [], |r| r.get(0)).unwrap();
        assert!(patch.contains("+Line two is added"));
        Ok(())
    }).unwrap();
}
