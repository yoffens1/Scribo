//! # File Logging Sink
//!
//! Outputs serialized JSON files representing completed tracing spans onto disk.

use crate::logging::types::{Sink, LogEvent, Trace};
use std::path::{Path, PathBuf};
use tokio::fs;
use std::pin::Pin;
use std::future::Future;

/// Sink that writes trace spans as pretty-printed JSON files inside a targeted folder.
pub struct FileSink {
    dir: PathBuf,
}

impl FileSink {
    /// Creates a new `FileSink` that writes traces under the specified path.
    pub fn new<P: AsRef<Path>>(dir: P) -> Self {
        Self {
            dir: dir.as_ref().to_path_buf(),
        }
    }
}

impl Sink for FileSink {
    fn write(&self, _event: &LogEvent) {}
    
    fn flush_trace(&self, trace: &Trace) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
        let dir = self.dir.clone();
        let trace = trace.clone();
        Box::pin(async move {
            let _ = fs::create_dir_all(&dir).await.map_err(|e| e.to_string())?;
            
            let filename = format!("trace-{:.0}-{}.json", trace.started_at, &trace.id[..std::cmp::min(8, trace.id.len())]);
            let file_path = dir.join(filename);
            
            let content = serde_json::to_string_pretty(&trace).map_err(|e| e.to_string())?;
            
            fs::write(&file_path, content).await.map_err(|e| e.to_string())?;
            
            println!("Trace saved: {:?}", file_path);
            Ok(())
        })
    }
}
