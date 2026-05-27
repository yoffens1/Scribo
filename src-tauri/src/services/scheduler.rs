use tokio::sync::mpsc;
use parking_lot::RwLock;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;

pub enum IndexCommand {
    IndexNote {
        note_id: i64,
        indexing_version: String,
        // Model used for embeddings
        model: String,
    },
    Shutdown,
}

#[derive(Clone)]
pub struct ReindexScheduler {
    sender: mpsc::Sender<IndexCommand>,
}

impl ReindexScheduler {
    pub fn new(pool: std::sync::Arc<RwLock<Option<Pool<SqliteConnectionManager>>>>) -> Self {
        let (tx, mut rx) = mpsc::channel::<IndexCommand>(100);
        
        let tx_clone = tx.clone();
        let pool_clone = pool.clone();
        
        // Spawn background task
        tauri::async_runtime::spawn(async move {
            while let Some(cmd) = rx.recv().await {
                match cmd {
                    IndexCommand::IndexNote { note_id, indexing_version, model } => {
                        println!("Processing indexing for note: {}", note_id);
                        
                        let pool_guard = pool_clone.read();
                        if let Some(ref p) = *pool_guard {
                            if let Ok(mut conn) = p.get() {
                                let needs_indexing = crate::services::validation::check_needs_indexing(
                                    &conn,
                                    note_id,
                                    &model,
                                    &indexing_version,
                                );

                                if let Ok(true) = needs_indexing {
                                    let dim = if model.contains("granite") { 384 } else { 1536 };
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

    pub async fn enqueue_note(&self, note_id: i64, indexing_version: String, model: String) {
        let _ = self.sender.send(IndexCommand::IndexNote {
            note_id,
            indexing_version,
            model,
        }).await;
    }
}
