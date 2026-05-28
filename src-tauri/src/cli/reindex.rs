//! # CLI Reindex Handler
//!
//! CLI reindex handler — runs model-drift detection and re-embeds all stale notes.

use std::path::Path;
use crate::ai::embedding::Embedder;
use crate::ai::types::EmbedderConfig;
use crate::services::reindex::{find_stale_notes, mark_stale_for_model_change};
use crate::services::indexer::{persist_indexed_file, IndexingPayload};
use crate::cli::query::make_state;

const CURRENT_MODEL: &str = "granite-embedding-97M-multilingual-r2-BF16";
const CURRENT_DIM: i64 = 384;
const INDEXING_VERSION: &str = "1";

pub fn handle_reindex(db_path: &Path, force: bool) {
    let state = match make_state(db_path) {
        Ok(s) => s,
        Err(e) => { eprintln!("DB open failed: {e}"); return; }
    };

    let stale = match find_stale_notes(&state, CURRENT_MODEL, CURRENT_DIM) {
        Ok(r) => r,
        Err(e) => { eprintln!("Stale check failed: {e}"); return; }
    };

    if stale.stale_notes.is_empty() && !force {
        println!("All notes already indexed with {} (dim={}).", CURRENT_MODEL, CURRENT_DIM);
        return;
    }

    let note_ids: Vec<i64> = if force {
        // take all notes
        let pool_guard = state.pool.read();
        let conn = pool_guard.as_ref().unwrap().get().unwrap();
        let mut stmt = conn.prepare("SELECT note_id FROM notes").unwrap();
        let rows = stmt.query_map([], |r| r.get::<_, i64>(0)).unwrap();
        rows.filter_map(Result::ok).collect()
    } else {
        let n = mark_stale_for_model_change(&state, CURRENT_MODEL, CURRENT_DIM).unwrap_or(0);
        println!("Marked {n} notes as stale.");
        stale.stale_notes.iter().map(|(id, _, _)| *id).collect()
    };

    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    rt.block_on(async {
        let embedder = Embedder::new(EmbedderConfig {
            provider: "local".to_string(),
            model: Some(CURRENT_MODEL.to_string()),
            api_key: None,
            base_url: None,
        });

        for (i, note_id) in note_ids.iter().enumerate() {
            // First, run persist_indexed_file to update the chunks structure and set notes indexing columns
            {
                let pool_guard = state.pool.read();
                let mut conn = pool_guard.as_ref().unwrap().get().unwrap();
                let _ = persist_indexed_file(&mut conn, IndexingPayload {
                    note_id: *note_id,
                    embedding_model: CURRENT_MODEL,
                    embedding_dim: CURRENT_DIM as u32,
                    indexing_version: INDEXING_VERSION,
                });
            }

            // Re-calculate embeddings for all fragments of this note.
            let texts: Vec<(i64, String)> = {
                let pool_guard = state.pool.read();
                let conn = pool_guard.as_ref().unwrap().get().unwrap();
                let mut stmt = conn.prepare(
                    "SELECT chunk_id, clean_text FROM chunks
                     WHERE note_id = ?1 AND level = 1"
                ).unwrap();
                let rows = stmt.query_map([note_id], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
                rows.filter_map(Result::ok).collect()
            };

            for (chunk_id, text) in texts {
                match embedder.embed(&text).await {
                    Ok(vec) => {
                        let bytes = bytemuck::cast_slice::<f32, u8>(&vec).to_vec();
                        let pool_guard = state.pool.read();
                        let conn = pool_guard.as_ref().unwrap().get().unwrap();
                        if let Err(e) = conn.execute(
                            "UPDATE chunks
                             SET embedding = ?1
                             WHERE chunk_id = ?2",
                            rusqlite::params![bytes, chunk_id],
                        ) {
                            eprintln!("Failed to update chunk {chunk_id} embedding in DB: {e}");
                        }
                    }
                    Err(e) => eprintln!("embed failed for chunk {chunk_id}: {e}"),
                }
            }

            println!("[{}/{}] reindexed note {}", i + 1, note_ids.len(), note_id);
        }
    });

    println!("\nDone. Model={}, dim={}", CURRENT_MODEL, CURRENT_DIM);

    // Clean up active models to prevent llama_cpp backend destruction order panic/abort.
    crate::ai::models::manager::get_model_manager().clear();
}
