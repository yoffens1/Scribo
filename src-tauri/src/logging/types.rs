//! # Logging Core Types
//!
//! Defines the logging severity levels, structured log events, trace spans, and the pluggable sink trait.

use serde::{Deserialize, Serialize};
use std::pin::Pin;
use std::future::Future;

/// Severity levels for logging.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    /// Verbose logs for developer debugging.
    Debug,
    /// Informational messages outlining normal operations.
    Info,
    /// Non-fatal issues that might require attention.
    Warn,
    /// Critical failures that halt operations.
    Error,
}

/// A structured log event containing metadata, severity, and custom payloads.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEvent {
    /// Trace ID grouping this event under a trace span (empty if outside trace).
    pub trace_id: String,
    /// UNIX timestamp of the event in milliseconds.
    pub timestamp: f64,
    /// Severity level of the event.
    pub level: LogLevel,
    /// Specific lifecycle stage or checkpoint name (e.g. "parser.start").
    pub stage: String,
    /// Human-readable log message.
    pub message: String,
    /// Custom key-value payload data for debugging.
    pub data: Option<serde_json::Value>,
    /// Optional execution duration of the stage in milliseconds.
    pub duration_ms: Option<f64>,
}

/// A trace span grouping a series of related `LogEvent`s.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Trace {
    /// Unique identifier for this trace span.
    pub id: String,
    /// Module namespace of the logger.
    pub namespace: String,
    /// Display label for the trace span.
    pub label: String,
    /// Start timestamp of the span in milliseconds.
    pub started_at: f64,
    /// End timestamp of the span in milliseconds.
    pub ended_at: Option<f64>,
    /// Cumulative duration of the trace in milliseconds.
    pub duration_ms: Option<f64>,
    /// Chronological list of events captured inside this trace.
    pub events: Vec<LogEvent>,
    /// Optional metadata dictionary attached to this trace.
    pub meta: Option<serde_json::Value>,
}

/// Interface for custom log writers/targets (e.g. console, file, local memory).
pub trait Sink: Send + Sync {
    /// Write an individual structured `LogEvent` immediately.
    fn write(&self, event: &LogEvent);

    /// Flush the entire completed `Trace` span to the destination target.
    fn flush_trace(&self, trace: &Trace) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>>;
}
