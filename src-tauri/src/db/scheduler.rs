use std::path::PathBuf;
use tokio::sync::mpsc;
use parking_lot::RwLock;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;

pub enum IndexCommand {
    IndexFile {
        file_path: String,
        content: String,
        chunking_version: String,
        // Model used for embeddings
        model: String,
    },
    IndexDirectory {
        dir_path: PathBuf,
        chunking_version: String,
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
                    IndexCommand::IndexFile { file_path, content, chunking_version, model } => {
                        println!("Processing indexing for: {}", file_path);
                        
                        let pool_guard = pool_clone.read();
                        if let Some(ref p) = *pool_guard {
                            if let Ok(mut conn) = p.get() {
                                let hash = crate::db::compute_file_hash(&content);
                                
                                let validation = crate::db::check_needs_indexing(
                                    &conn,
                                    &file_path,
                                    &hash,
                                    &model,
                                    &chunking_version,
                                    None
                                );

                                if let Ok(val) = validation {
                                    if val.should_index {
                                        // TODO: generate actual embeddings
                                        let file_name = std::path::Path::new(&file_path)
                                            .file_name()
                                            .and_then(|n| n.to_str())
                                            .unwrap_or("unknown");
                                            
                                        let payload = crate::db::IndexingPayload {
                                            file_path: &file_path,
                                            file_name,
                                            file_hash: &hash,
                                            mtime: None,
                                            embedding_model: &model,
                                            embedding_dim: 1536,
                                            chunking_version: &chunking_version,
                                            chunks: vec![], // TODO: generate via chunker and LLM
                                        };
                                        
                                        if let Err(e) = crate::db::persist_indexed_file(&mut conn, payload) {
                                            eprintln!("Failed to persist indexed file {}: {}", file_path, e);
                                        }
                                    } else {
                                        println!("Skipping indexing for {}: unchanged", file_path);
                                    }
                                }
                            }
                        }
                    }
                    IndexCommand::IndexDirectory { dir_path, chunking_version: _, model: _ } => {
                        println!("Scanning directory for indexing: {:?}", dir_path);
                        // TODO: Recursively queue files
                    }
                    IndexCommand::Shutdown => break,
                }
            }
        });

        Self { sender: tx_clone }
    }

    pub async fn enqueue_file(&self, file_path: String, content: String, chunking_version: String, model: String) {
        let _ = self.sender.send(IndexCommand::IndexFile {
            file_path,
            content,
            chunking_version,
            model,
        }).await;
    }
}
