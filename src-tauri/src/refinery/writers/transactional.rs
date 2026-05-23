use std::collections::HashMap;
use tokio::fs;
use crate::DbState;
use crate::refinery::types::WriteOperation;
use super::file_writer::FileWriter;

pub struct TransactionalWriter {
    writer: FileWriter,
    merge_backups: HashMap<String, String>,
    executed: Vec<WriteOperation>,
}

impl TransactionalWriter {
    pub fn new(writer: FileWriter) -> Self {
        Self {
            writer,
            merge_backups: HashMap::new(),
            executed: Vec::new(),
        }
    }

    pub async fn execute_batch(&mut self, operations: Vec<WriteOperation>, source_file_id: Option<i64>, db_state: Option<&DbState>) -> Result<(), String> {
        self.executed.clear();
        self.merge_backups.clear();

        for op in &operations {
            match op {
                WriteOperation::MergeChunk { target_file, .. } => {
                    if let Ok(content) = fs::read_to_string(target_file).await {
                        self.merge_backups.insert(target_file.clone(), content);
                    }
                }
                WriteOperation::DeleteFile { path } => {
                    if let Ok(content) = fs::read_to_string(path).await {
                        self.merge_backups.insert(path.clone(), content);
                    }
                }
                _ => {}
            }

            if let Err(e) = self.writer.execute(op, source_file_id, db_state).await {
                self.rollback().await;
                return Err(e);
            }
            self.executed.push(op.clone());
        }
        Ok(())
    }

    async fn rollback(&mut self) {
        for op in self.executed.iter().rev() {
            match op {
                WriteOperation::CreateFile { path, .. } => {
                    let _ = fs::remove_file(path).await;
                }
                WriteOperation::MergeChunk { target_file, .. } => {
                    if let Some(backup) = self.merge_backups.get(target_file) {
                        let _ = fs::write(target_file, backup).await;
                    } else {
                        let _ = fs::remove_file(target_file).await;
                    }
                }
                WriteOperation::CreateFolder { .. } => {}
                WriteOperation::MoveFile { from, to } => {
                    let _ = fs::rename(to, from).await;
                }
                WriteOperation::DeleteFile { path } => {
                    if let Some(backup) = self.merge_backups.get(path) {
                        let _ = fs::write(path, backup).await;
                    }
                }
            }
        }
    }
}
