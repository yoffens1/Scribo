use serde::{Deserialize, Serialize};
use std::pin::Pin;
use std::future::Future;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEvent {
    pub trace_id: String,
    pub timestamp: f64,
    pub level: LogLevel,
    pub stage: String,
    pub message: String,
    pub data: Option<serde_json::Value>,
    pub duration_ms: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Trace {
    pub id: String,
    pub namespace: String,
    pub label: String,
    pub started_at: f64,
    pub ended_at: Option<f64>,
    pub duration_ms: Option<f64>,
    pub events: Vec<LogEvent>,
    pub meta: Option<serde_json::Value>,
}

pub trait Sink: Send + Sync {
    fn write(&self, event: &LogEvent);
    fn flush_trace(&self, trace: &Trace) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>>;
}
