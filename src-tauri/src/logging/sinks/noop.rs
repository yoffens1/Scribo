//! # No-Op Logging Sink
//!
//! Discards all incoming events and traces. Primarily used to disable logs during testing.

use crate::logging::types::{Sink, LogEvent, Trace};
use std::pin::Pin;
use std::future::Future;

/// Sink that silently discards all log events and completes flush requests immediately.
pub struct NoopSink;

impl Sink for NoopSink {
    fn write(&self, _event: &LogEvent) {}
    fn flush_trace(&self, _trace: &Trace) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
        Box::pin(async { Ok(()) })
    }
}
