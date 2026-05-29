//! # Retrieval Parameter Calibration Service
//!
//! Maintains an evaluation dataset of query-note relevance pairs in the database,
//! and runs grid-search optimization to find optimal hybrid retrieval parameters
//! (embedding_weight, rrf_k, and min_score) to maximize Mean Reciprocal Rank (MRR).

pub mod dataset;
pub mod prefetch;

pub use dataset::{add_calibration_pair, seed_calibration_dataset, CalibrationPair, CalibrationNote};

use rusqlite::params;
use std::collections::{HashMap, HashSet};
use crate::db::state::DbState;
use crate::error::AppError;
use crate::ai::embedding::Embedder;
use crate::ai::types::EmbedderConfig;
use crate::retrieval::calibration::EvalSample;
use prefetch::prefetch_hits;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalibrationReport {
    pub total_pairs: usize,
    pub initial_embedding_weight: f32,
    pub initial_rrf_k: f32,
    pub initial_mrr: f32,
    pub optimal_embedding_weight: f32,
    pub optimal_rrf_k: f32,
    pub optimal_min_score: f32,
    pub optimal_mrr: f32,
}

/// Runs grid search to optimize RRF parameters on the calibration dataset.
/// Saves the best values in the `meta` table.
pub async fn run_calibration(state: &DbState) -> Result<CalibrationReport, AppError> {
    let mut queries_path = std::path::PathBuf::from("data/calibration_queries.json");
    let mut notes_path = std::path::PathBuf::from("data/calibration_notes.json");

    if !queries_path.exists() {
        let mut p = std::path::PathBuf::from("src-tauri");
        p.push("data");
        p.push("calibration_queries.json");
        if p.exists() {
            queries_path = p;
        }
    }
    if !notes_path.exists() {
        let mut p = std::path::PathBuf::from("src-tauri");
        p.push("data");
        p.push("calibration_notes.json");
        if p.exists() {
            notes_path = p;
        }
    }

    let use_isolated_env = queries_path.exists() && notes_path.exists();

    let pairs = if use_isolated_env {
        println!("Loading isolated calibration notes from {}", notes_path.display());
        println!("Loading isolated calibration queries from {}", queries_path.display());

        let queries_content = std::fs::read_to_string(&queries_path)
            .map_err(|e| AppError::Other(e.to_string()))?;
        let file_pairs: Vec<CalibrationPair> = serde_json::from_str(&queries_content)
            .map_err(|e| AppError::Other(e.to_string()))?;

        file_pairs
    } else {
        // Fallback: 0. Try loading custom queries from local JSON file first
        if queries_path.exists() {
            println!("Loading calibration queries from {}", queries_path.display());
            let content = std::fs::read_to_string(&queries_path).map_err(|e| AppError::Other(e.to_string()))?;
            let file_pairs: Vec<CalibrationPair> = serde_json::from_str(&content).map_err(|e| AppError::Other(e.to_string()))?;
            state.with_conn(|conn| {
                conn.execute("DELETE FROM retrieval_calibration", [])?;
                for p in &file_pairs {
                    conn.execute(
                        "INSERT OR IGNORE INTO retrieval_calibration (query, expected_note_title, relevance_weight) VALUES (?, ?, ?)",
                        params![p.query.trim(), p.expected_note_title.trim(), p.relevance_weight],
                    )?;
                }
                Ok::<(), AppError>(())
            })?;
        }

        // 1. Load dataset
        let pairs = state.with_conn(|conn| {
            let mut stmt = conn.prepare("SELECT query, expected_note_title, relevance_weight FROM retrieval_calibration")?;
            let rows = stmt.query_map([], |row| {
                Ok(CalibrationPair {
                    query: row.get(0)?,
                    expected_note_title: row.get(1)?,
                    relevance_weight: row.get(2)?,
                })
            })?;
            let res: Result<Vec<_>, rusqlite::Error> = rows.collect();
            Ok(res.map_err(AppError::from)?)
        })?;

        // Seed if empty
        let pairs = if pairs.is_empty() {
            seed_calibration_dataset(state)?;
            state.with_conn(|conn| {
                let mut stmt = conn.prepare("SELECT query, expected_note_title, relevance_weight FROM retrieval_calibration")?;
                let rows = stmt.query_map([], |row| {
                    Ok(CalibrationPair {
                        query: row.get(0)?,
                        expected_note_title: row.get(1)?,
                        relevance_weight: row.get(2)?,
                    })
                })?;
                let res: Result<Vec<_>, rusqlite::Error> = rows.collect();
                Ok(res.map_err(AppError::from)?)
            })?
        } else {
            pairs
        };

        pairs
    };

    if pairs.is_empty() {
        return Err(AppError::Other("Calibration dataset is empty and no notes found to seed from.".to_string()));
    }

    // 2. Precompute embeddings & prefetch FTS5 + Vector results for all unique queries
    let mut unique_queries = HashSet::new();
    for p in &pairs {
        unique_queries.insert(p.query.clone());
    }

    let mut query_embeddings = HashMap::new();
    for q in &unique_queries {
        if let Ok(emb) = crate::retrieval::embed_query(state, q).await {
            query_embeddings.insert(q.clone(), emb);
        }
    }

    // Setup DB connection and prefetch search results
    let mut prefetched = Vec::new();

    if use_isolated_env {
        let embedder = Embedder::new(EmbedderConfig {
            provider: "local".to_string(),
            model: Some(crate::ai::embedding::CURRENT_EMBEDDING_MODEL.to_string()),
            api_key: None,
            base_url: None,
        });

        let mut mem_conn = rusqlite::Connection::open_in_memory()
            .map_err(|e| AppError::Other(e.to_string()))?;
        crate::db::schema::tables::create_schema(&mem_conn)?;

        // Load isolated notes
        let notes_content = std::fs::read_to_string(&notes_path)
            .map_err(|e| AppError::Other(e.to_string()))?;
        let file_notes: Vec<CalibrationNote> = serde_json::from_str(&notes_content)
            .map_err(|e| AppError::Other(e.to_string()))?;

        println!("Indexing {} isolated notes in memory...", file_notes.len());
        for note in &file_notes {
            // Insert note
            let note_id = crate::db::repos::notes::insert(&mem_conn, &crate::domain::note::NewNote {
                title: note.title.clone(),
                content: note.content.clone(),
                lifecycle: Some(crate::domain::note::NoteLifecycle::Active),
                ..Default::default()
            })?;

            // Fragment/index note
            let payload = crate::services::indexer::IndexingPayload {
                note_id: note_id.0,
                embedding_model: crate::ai::embedding::CURRENT_EMBEDDING_MODEL,
                embedding_dim: crate::ai::embedding::CURRENT_DIM as u32,
                indexing_version: "1",
            };
            crate::services::indexer::persist_indexed_file(&mut mem_conn, payload)?;

            // Embed fragments
            let fragments = crate::db::repos::fragments::list_by_note(&mem_conn, note_id.0, crate::ai::embedding::CURRENT_EMBEDDING_MODEL)?;
            for frag in &fragments {
                let emb = embedder.embed(&frag.text_clean).await?;
                let emb_bytes = bytemuck::cast_slice::<f32, u8>(&emb);
                crate::db::repos::fragments::set_embedding(&mem_conn, note_id.0, frag.fragment_index, emb_bytes, crate::ai::embedding::CURRENT_EMBEDDING_MODEL, "1")?;
            }
        }

        // Prefetch search results from in-memory DB
        for p in &pairs {
            let (keyword_hits, vector_hits) = prefetch_hits(&mem_conn, &p.query, query_embeddings.get(&p.query).map(|v| v.as_slice()));
            prefetched.push(EvalSample {
                expected_title: p.expected_note_title.clone(),
                weight: p.relevance_weight,
                keyword_hits,
                vector_hits,
            });
        }
    } else {
        // Prefetch search results from actual DB
        for p in &pairs {
            let (keyword_hits, vector_hits) = state.with_conn(|conn| {
                Ok(prefetch_hits(conn, &p.query, query_embeddings.get(&p.query).map(|v| v.as_slice())))
            }).unwrap_or((Vec::new(), Vec::new()));

            prefetched.push(EvalSample {
                expected_title: p.expected_note_title.clone(),
                weight: p.relevance_weight,
                keyword_hits,
                vector_hits,
            });
        }
    }

    // Evaluate initial baseline (default weight=1.5, k=60.0)
    let initial_mrr = crate::retrieval::calibration::mean_reciprocal_rank(&prefetched, 1.5, 60.0);

    // Grid search
    let grid_params = crate::retrieval::calibration::GridSearchParameters::default();
    let (best_w, best_k, best_mrr) = crate::retrieval::calibration::grid_search(&prefetched, &grid_params);

    // Calibrate min_score
    let calibrated_min_score = crate::retrieval::calibration::calibrate_min_score(&prefetched, best_w, best_k);

    // 5. Save optimal values to database meta table
    state.with_conn(|conn| {
        conn.execute(
            "INSERT INTO meta (key, value) VALUES ('retrieval_embedding_weight', ?)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![best_w.to_string()],
        )?;
        conn.execute(
            "INSERT INTO meta (key, value) VALUES ('retrieval_rrf_k', ?)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![best_k.to_string()],
        )?;
        conn.execute(
            "INSERT INTO meta (key, value) VALUES ('retrieval_min_score', ?)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![calibrated_min_score.to_string()],
        )?;
        Ok(())
    })?;

    // Clear LLM models to prevent destruction order abort
    crate::ai::models::manager::get_model_manager().clear();

    Ok(CalibrationReport {
        total_pairs: pairs.len(),
        initial_embedding_weight: 1.5,
        initial_rrf_k: 60.0,
        initial_mrr,
        optimal_embedding_weight: best_w,
        optimal_rrf_k: best_k,
        optimal_min_score: calibrated_min_score,
        optimal_mrr: best_mrr,
    })
}
