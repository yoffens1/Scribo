//! # In-Memory Logging Sink
//!
//! Stores trace spans in a rolling thread-safe in-memory vector, primarily for inspection and UI visualization.

use crate::logging::types::{Sink, LogEvent, Trace};
use parking_lot::Mutex;
use std::pin::Pin;
use std::future::Future;

/// Sink that buffers completed traces in a rolling, bounded in-memory list.
pub struct MemorySink {
    traces: Mutex<Vec<Trace>>,
    max_traces: usize,
}

impl MemorySink {
    /// Creates a new `MemorySink` with a specified trace capacity limit.
    pub fn new(max_traces: usize) -> Self {
        Self {
            traces: Mutex::new(Vec::new()),
            max_traces,
        }
    }
    
    /// Retrieves a cloned list of all buffered traces.
    pub fn get_all(&self) -> Vec<Trace> {
        self.traces.lock().clone()
    }
    
    /// Clears all buffered traces in memory.
    pub fn clear(&self) {
        self.traces.lock().clear();
    }
}

impl Default for MemorySink {
    fn default() -> Self {
        Self::new(50)
    }
}

impl Sink for MemorySink {
    fn write(&self, _event: &LogEvent) {}
    
    fn flush_trace(&self, trace: &Trace) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
        let mut t = self.traces.lock();
        t.push(trace.clone());
        if t.len() > self.max_traces {
            t.remove(0);
        }
        Box::pin(async { Ok(()) })
    }
}
