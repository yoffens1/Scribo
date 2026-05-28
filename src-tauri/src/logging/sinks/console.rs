//! # Console Logging Sink
//!
//! Outputs namespaced log events directly to standard output or standard error depending on severity.

use crate::logging::types::{Sink, LogEvent, Trace, LogLevel};
use std::pin::Pin;
use std::future::Future;

/// Sink that writes formatted logs directly to the standard terminal stream.
pub struct ConsoleSink;

impl Sink for ConsoleSink {
    fn write(&self, e: &LogEvent) {
        let prefix = if !e.trace_id.is_empty() {
            format!("[{}] ", &e.trace_id[..std::cmp::min(8, e.trace_id.len())])
        } else {
            "".to_string()
        };
        let stage = format!("[{}]", e.stage);
        let dur = if let Some(d) = e.duration_ms {
            format!(" ({:.0}ms)", d)
        } else {
            "".to_string()
        };
        
        let msg = format!("{}{}{} {}", prefix, stage, dur, e.message);
        let data_str = e.data.as_ref().map(|d| format!(" data: {:?}", d)).unwrap_or_default();

        match e.level {
            LogLevel::Error => eprintln!("[ERROR] {}{}", msg, data_str),
            LogLevel::Warn => println!("[WARN] {}{}", msg, data_str),
            LogLevel::Info => println!("[INFO] {}{}", msg, data_str),
            LogLevel::Debug => println!("[DEBUG] {}{}", msg, data_str),
        }
    }
    
    fn flush_trace(&self, _trace: &Trace) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
        Box::pin(async { Ok(()) })
    }
}
