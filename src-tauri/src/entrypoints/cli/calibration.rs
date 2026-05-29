//! # CLI Calibration Handler
//!
//! Subcommands for calibrating the hybrid retrieval system parameters and managing evaluation query aliases.

use std::path::Path;
use crate::DbState;
use crate::services::calibration::{run_calibration, add_calibration_pair};

/// Runs the grid search calibration process on the current database
/// and displays the results before and after calibration.
pub fn handle_calibrate(db_path: &Path) {
    println!("Initializing database state at {:?}", db_path);
    let manager = r2d2_sqlite::SqliteConnectionManager::file(db_path);
    let pool = r2d2::Pool::new(manager).expect("Failed to create DB pool");
    let state = DbState::new();
    *state.pool.write() = Some(pool);

    println!("Starting retrieval parameter calibration...");
    println!("This will pre-compute embeddings and evaluate multiple parameter sets. Please wait...");

    // Run async calibration on tokio block
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to build Tokio runtime");

    match rt.block_on(run_calibration(&state)) {
        Ok(report) => {
            println!("\n==================================================");
            println!("       RETRIEVAL CALIBRATION COMPLETED");
            println!("==================================================");
            println!("Total evaluation query pairs: {}", report.total_pairs);
            println!();
            println!("Baseline Parameters (Default):");
            println!("  Embedding weight:  {:.2}", report.initial_embedding_weight);
            println!("  RRF constant k:    {:.2}", report.initial_rrf_k);
            println!("  Term boost weight: {:.2}", report.initial_term_boost_weight);
            println!("  Mean Reciprocal Rank (MRR): {:.2}%", report.initial_mrr * 100.0);
            println!();
            println!("Calibrated Parameters (Optimal):");
            println!("  Embedding weight:  {:.2}  (vector vs keyword importance)", report.optimal_embedding_weight);
            println!("  RRF constant k:    {:.2}  (position dampening factor)", report.optimal_rrf_k);
            println!("  Term boost weight: {:.2}  (exact keyword match boost factor)", report.optimal_term_boost_weight);
            println!("  Hard min_score:    {:.2}%  (dynamic garbage threshold)", report.optimal_min_score * 100.0);
            println!("  Mean Reciprocal Rank (MRR): {:.2}%  ({:+.1}% improvement)", 
                report.optimal_mrr * 100.0, 
                if report.initial_mrr > 0.0 { (report.optimal_mrr - report.initial_mrr) / report.initial_mrr * 100.0 } else { 0.0 }
            );
            println!("==================================================");
            println!("Settings successfully saved to the 'meta' table!");
        }
        Err(e) => {
            eprintln!("Calibration failed: {:?}", e);
        }
    }
}

/// Adds a custom query alias target to the calibration dataset.
pub fn handle_add_alias(db_path: &Path, query: &str, target: &str, relevance: f32) {
    let manager = r2d2_sqlite::SqliteConnectionManager::file(db_path);
    let pool = r2d2::Pool::new(manager).expect("Failed to create DB pool");
    let state = DbState::new();
    *state.pool.write() = Some(pool);

    println!("Adding alias: {:?} -> {:?} (relevance={:.2})", query, target, relevance);
    match add_calibration_pair(&state, query, target, relevance) {
        Ok(_) => {
            println!("Successfully added alias/evaluation target to the dataset.");
        }
        Err(e) => {
            eprintln!("Failed to add alias: {:?}", e);
        }
    }
}
