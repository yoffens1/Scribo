//! # CLI Query Handler
//!
//! Subcommand for hybrid retrieval (FTS5 + vector + RRF) from the terminal.
//! Reuses the same pipeline as the Tauri `retrieval_query` command.

use std::path::Path;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;

use crate::DbState;
use crate::ai::embedding::Embedder;
use crate::ai::types::EmbedderConfig;
use crate::retrieval::{
    retrieve, RetrievalConfig, RetrieveOptions,
};
use crate::retrieval::types::{
    RetrievalMode, PipelineConfig, AiRerankConfig, RerankMode, SynonymExpansion, RetrieveFilters,
};

/// Builds a `DbState` whose pool already points at `db_path`,
/// without going through the Tauri `db_initialize` command.
pub fn make_state(db_path: &Path) -> Result<DbState, String> {
    let manager = SqliteConnectionManager::file(db_path).with_init(|conn| {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA temp_store = MEMORY;
             PRAGMA busy_timeout = 5000;",
        )
    });
    let pool: Pool<SqliteConnectionManager> = Pool::builder()
        .max_size(4)
        .build(manager)
        .map_err(|e| e.to_string())?;

    let state = DbState::new();
    *state.pool.write() = Some(pool);
    Ok(state)
}

/// Runs a retrieval query and prints ranked hits to stdout.
pub fn handle_query(
    db_path: &Path,
    query: &str,
    top_k: usize,
    mode: RetrievalMode,
    hyde: bool,
    rerank: bool,
    expand: bool,
    min_score: f32,
) {
    let state = match make_state(db_path) {
        Ok(s) => s,
        Err(e) => { eprintln!("Failed to open DB: {e}"); return; }
    };

    let mut resolved_min_score = min_score;
    if min_score == 0.005 {
        if let Ok(db_min) = state.with_conn(|conn| {
            let mut val_str: Option<String> = None;
            if let Ok(mut stmt) = conn.prepare("SELECT value FROM meta WHERE key = 'retrieval_min_score'") {
                val_str = stmt.query_row([], |row| row.get::<_, String>(0)).ok();
            }
            Ok(val_str)
        }) {
            if let Some(s) = db_min.and_then(|v| v.parse::<f32>().ok()) {
                resolved_min_score = s;
            }
        }
    }

    const CURRENT_MODEL: &str = crate::constants::EMBEDDING_MODEL;
    const CURRENT_DIM: i64 = crate::constants::EMBEDDING_DIM as i64;

    if let Ok(report) = crate::services::reindex::find_stale_notes(&state, CURRENT_MODEL, CURRENT_DIM) {
        if !report.stale_notes.is_empty() {
            eprintln!(
                "[query] WARNING: {} notes use a different embedding model. \
                 Their vectors will be ignored. Run `reindex` to fix.",
                report.stale_notes.len()
            );
        }
    }

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");

    rt.block_on(async {
        // 1. Embed the query locally — only needed for Embedding/Hybrid modes.
        let query_embedding = match mode {
            RetrievalMode::Keyword => None,
            _ => {
                let embedder = Embedder::new(EmbedderConfig {
                    provider: "local".to_string(),
                    model: Some(CURRENT_MODEL.to_string()),
                    api_key: None,
                    base_url: None,
                });
                match embedder.embed_query(query).await {
                    Ok(emb) => Some(emb),
                    Err(e) => {
                        eprintln!("[query] WARNING: failed to embed query: {e}. Falling back to FTS5 only.");
                        None
                    }
                }
            }
        };

        // 2. Set up retrieval config
        let config = RetrievalConfig {
            mode,
            embedding_weight: None, // Will load from DB calibrated value if calibrated
            pipeline: Some(PipelineConfig {
                auto_translate: Some(false),
                expand_synonyms: Some(if expand {
                    SynonymExpansion::Static
                } else {
                    SynonymExpansion::Off
                }),
                synonym_dict: None,
                hyde: Some(hyde),
            }),
            ai_rerank: Some(AiRerankConfig {
                enabled: rerank,
                mode: Some(RerankMode::Listwise),
                max_candidates: Some(25),
            }),
            vault_lang: None,
            llm_config: None,
            adaptive_weights: None,
            tuning: None, // Will load from DB calibrated values if calibrated
        };

        let opts = RetrieveOptions {
            top_k: Some(top_k * 3), // fetch more for local filtering/reranking
            filters: Some(RetrieveFilters {
                note_id: None,
            }),
            target_level: Some(1),
            explain: Some(true),
            ..Default::default()
        };

        // 3. Run retrieval.
        let results = match retrieve(&state, query, query_embedding.as_deref(), &config, &opts).await {
            Ok(r) => r,
            Err(e) => { eprintln!("Retrieve failed: {e}"); return; }
        };

        // Hard threshold filter and truncate to top_k
        let filtered: Vec<_> = results
            .into_iter()
            .filter(|r| r.score >= resolved_min_score)
            .take(top_k)
            .collect();

        if filtered.is_empty() {
            println!("No confident results for {:?} (min_score={:.1}%)", query, resolved_min_score * 100.0);
            return;
        }

        let max_score = filtered.first().map(|r| r.score).unwrap_or(0.0);

        println!("Top {} results for {:?} (mode = {:?}, min_score = {:.1}%):\n", filtered.len(), query, mode, resolved_min_score * 100.0);
        for (i, r) in filtered.iter().enumerate() {
            let note_title = r.note_title.as_deref().unwrap_or("Untitled");
            let preview = r.text.as_deref().unwrap_or("").trim().replace('\n', " ");
            let preview: String = preview.chars().take(180).collect();

            let rel_percentage = if max_score > 0.0 {
                (r.score / max_score) * 100.0
            } else {
                0.0
            };

            println!(
                "{:>2}. [Relevance: {:.2}%]  Note: \"{}\" (ID: {}, Fragment: {})",
                i + 1,
                rel_percentage,
                note_title,
                r.fragment_ref.note_id.0,
                r.fragment_ref.fragment_index,
            );
            println!("    \"{}\"", preview);

            if let Some(ref dbg) = r.debug {
                let bm25_str = dbg.bm25_rank.map(|rk| format!("#{}", rk + 1)).unwrap_or_else(|| "N/A".to_string());
                let vector_str = dbg.vector_rank.map(|rk| format!("#{}", rk + 1)).unwrap_or_else(|| "N/A".to_string());

                let total_parts = dbg.rrf_score + dbg.term_boost;
                let rrf_contrib = if total_parts > 0.0 { (dbg.rrf_score / total_parts) * 100.0 } else { 0.0 };
                let boost_contrib = if total_parts > 0.0 { (dbg.term_boost / total_parts) * 100.0 } else { 0.0 };

                println!(
                    "    Breakdown: RRF Rank-score Contribution = {:.2}%, Lexical Boost Contribution = {:.2}%",
                    rrf_contrib,
                    boost_contrib,
                );
                print!("    Diagnostic: BM25 Rank = {}, Vector Rank = {}, Raw Score = {:.2}%", bm25_str, vector_str, r.score * 100.0);
                if let Some(rerank) = dbg.rerank_score {
                    print!(", Rerank Score = {:.2}%", rerank * 100.0);
                }
                println!("\n");
            } else {
                println!();
            }
        }
    });

    // Clean up active models to prevent llama_cpp backend destruction order panic/abort.
    crate::ai::models::manager::get_model_manager().clear();
}
