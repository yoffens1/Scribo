use crate::logging::types::{Sink, LogEvent, Trace};
use parking_lot::Mutex;
use std::pin::Pin;
use std::future::Future;

pub struct MemorySink {
    traces: Mutex<Vec<Trace>>,
    max_traces: usize,
}

impl MemorySink {
    pub fn new(max_traces: usize) -> Self {
        Self {
            traces: Mutex::new(Vec::new()),
            max_traces,
        }
    }
    
    pub fn get_all(&self) -> Vec<Trace> {
        self.traces.lock().clone()
    }
    
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
