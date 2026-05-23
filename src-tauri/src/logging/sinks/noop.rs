use crate::logging::types::{Sink, LogEvent, Trace};
use std::pin::Pin;
use std::future::Future;

pub struct NoopSink;

impl Sink for NoopSink {
    fn write(&self, _event: &LogEvent) {}
    fn flush_trace(&self, _trace: &Trace) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
        Box::pin(async { Ok(()) })
    }
}
