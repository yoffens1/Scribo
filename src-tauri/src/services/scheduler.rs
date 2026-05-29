//! # Scheduler Service
//!
//! Provides a lightweight async work queue for background note re-indexing.
//!
//! ## Design
//!
//! `ReindexScheduler` wraps a `tokio::sync::mpsc` channel with a capacity of 100 messages.
//! On construction it spawns a single long-lived Tauri async task that drains the channel
//! sequentially. Callers push [`IndexCommand::IndexNote`] messages via [`enqueue_note`](ReindexScheduler::enqueue_note);
//! the background task checks whether the note actually needs re-indexing before committing
//! any database work.
//!
//! ## Why sequential?
//!
//! SQLite allows only one concurrent writer. A single consumer task avoids write contention
//! without additional locking. For the expected workload (tens of notes per session) the
//! throughput is more than adequate.

use tokio::sync::mpsc;
use parking_lot::RwLock;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;

/// Commands understood by the background indexing worker.
pub enum IndexCommand {
    /// Request indexing of a specific note.
    IndexNote {
        note_id: i64,
        /// Semantic version of the indexing pipeline — used by the validation step
        /// to detect when a note's index was built with an older pipeline version.
        indexing_version: String,
        /// Embedding model identifier (e.g. `"nomic-embed-text"`).
        model: String,
    },
    /// Signals the worker to drain the queue and exit cleanly.
    Shutdown,
}

/// A cloneable handle to the background indexing queue.
///
/// Cheaply cloneable — multiple Tauri commands can hold a copy and enqueue work concurrently.
#[derive(Clone)]
pub struct ReindexScheduler {
    sender: mpsc::Sender<IndexCommand>,
}

impl ReindexScheduler {
    /// Creates a new scheduler and immediately spawns the background worker task.
    ///
    /// The worker has exclusive access to the SQLite connection pool and processes
    /// [`IndexCommand`]s one at a time until [`IndexCommand::Shutdown`] is received.
    pub fn new(pool: std::sync::Arc<RwLock<Option<Pool<SqliteConnectionManager>>>>) -> Self {
        let (tx, mut rx) = mpsc::channel::<IndexCommand>(100);

        let tx_clone = tx.clone();
        let pool_clone = pool.clone();

        // Long-lived background task — lives for the duration of the Tauri app process.
        tauri::async_runtime::spawn(async move {
            while let Some(cmd) = rx.recv().await {
                match cmd {
                    IndexCommand::IndexNote { note_id, indexing_version, model } => {
                        println!("Processing indexing for note: {}", note_id);

                        let pool_guard = pool_clone.read();
                        if let Some(ref p) = *pool_guard {
                            if let Ok(mut conn) = p.get() {
                                // Check whether the note actually needs work before writing anything.
                                let needs_indexing = crate::services::validation::check_needs_indexing(
                                    &conn,
                                    note_id,
                                    &model,
                                    &indexing_version,
                                );

                                if let Ok(true) = needs_indexing {
                                    // Infer embedding dimension from model name heuristic.
                                    // Use centralized constant — avoids stale heuristics when the model changes.
                                    let dim = crate::constants::EMBEDDING_DIM as u32;
                                    let payload = crate::services::indexer::IndexingPayload {
                                        note_id,
                                        embedding_model: &model,
                                        embedding_dim: dim,
                                        indexing_version: &indexing_version,
                                    };

                                    if let Err(e) = crate::services::indexer::persist_indexed_file(&mut conn, payload) {
                                        eprintln!("Failed to persist indexed note {}: {}", note_id, e);
                                    }
                                } else {
                                    println!("Skipping indexing for note {}: unchanged or up-to-date", note_id);
                                }
                            }
                        }
                    }
                    IndexCommand::Shutdown => break,
                }
            }
        });

        Self { sender: tx_clone }
    }

    /// Enqueues a note for background re-indexing. Returns immediately (fire-and-forget).
    /// If the channel is full (capacity 100), the send is silently dropped.
    pub async fn enqueue_note(&self, note_id: i64, indexing_version: String, model: String) {
        let _ = self.sender.send(IndexCommand::IndexNote {
            note_id,
            indexing_version,
            model,
        }).await;
    }
}
