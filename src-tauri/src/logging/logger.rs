//! # Structural Logger
//!
//! Implements namespaced logger structs supporting trace state tracking and structured logging.

use crate::logging::types::{LogLevel, LogEvent, Trace, Sink};
use std::sync::Arc;
use parking_lot::Mutex;
use std::time::{SystemTime, UNIX_EPOCH, Instant};

/// A thread-safe logger tied to a hierarchical namespace.
/// Can group log messages under a `Trace` span to measure duration and retain context.
pub struct Logger {
    /// Namespace identifier (e.g. "scribo.ai.embedding").
    pub namespace: String,
    current_trace: Mutex<Option<Trace>>,
    trace_instant: Mutex<Option<Instant>>,
    sinks: Vec<Arc<dyn Sink>>,
    enabled: bool,
    min_level: LogLevel,
}

impl Logger {
    /// Creates a new `Logger` instance.
    pub fn new(
        namespace: String,
        enabled: bool,
        min_level: LogLevel,
        sinks: Vec<Arc<dyn Sink>>,
    ) -> Self {
        Self {
            namespace,
            current_trace: Mutex::new(None),
            trace_instant: Mutex::new(None),
            sinks,
            enabled,
            min_level,
        }
    }

    /// Spawns a child logger inheriting this logger's configuration, with a nested namespace.
    pub fn child(&self, sub_namespace: &str) -> Self {
        Self::new(
            format!("{}.{}", self.namespace, sub_namespace),
            self.enabled,
            self.min_level,
            self.sinks.clone(),
        )
    }

    /// Generates a reasonably unique trace ID without needing external dependencies.
    fn generate_trace_id() -> String {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        format!("{:x}-{:x}", now.as_secs(), now.subsec_nanos())
    }

    /// Gets current epoch timestamp in milliseconds.
    fn get_now_ms() -> f64 {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        now.as_secs_f64() * 1000.0 + now.subsec_nanos() as f64 / 1_000_000.0
    }

    /// Starts a new trace span with a descriptive label and optional metadata.
    /// Returns the trace ID.
    pub fn start_trace(&self, label: &str, meta: Option<serde_json::Value>) -> String {
        if !self.enabled {
            return String::new();
        }
        let id = Self::generate_trace_id();
        let started_at = Self::get_now_ms();
        
        let trace = Trace {
            id: id.clone(),
            namespace: self.namespace.clone(),
            label: label.to_string(),
            started_at,
            ended_at: None,
            duration_ms: None,
            events: Vec::new(),
            meta,
        };

        *self.current_trace.lock() = Some(trace);
        *self.trace_instant.lock() = Some(Instant::now());

        self.log(LogLevel::Info, "trace.start", label, None, None);
        id
    }

    /// Logs a structured event at a specified level, stage, and duration.
    /// Also appends the event to the active trace span if one is currently open.
    pub fn log(
        &self,
        level: LogLevel,
        stage: &str,
        message: &str,
        data: Option<serde_json::Value>,
        duration_ms: Option<f64>,
    ) {
        if !self.enabled || level < self.min_level {
            return;
        }

        let mut trace_guard = self.current_trace.lock();
        let trace_id = trace_guard.as_ref().map(|t| t.id.clone()).unwrap_or_default();

        let event = LogEvent {
            trace_id,
            timestamp: Self::get_now_ms(),
            level,
            stage: stage.to_string(),
            message: message.to_string(),
            data,
            duration_ms,
        };

        if let Some(ref mut trace) = *trace_guard {
            trace.events.push(event.clone());
        }

        for sink in &self.sinks {
            sink.write(&event);
        }
    }

    /// Automatically logs duration and errors of a asynchronous closure.
    pub async fn time<T, F, Fut>(&self, stage: &str, f: F, data: Option<serde_json::Value>) -> Result<T, String>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, String>>,
    {
        let t0 = Instant::now();
        match f().await {
            Ok(result) => {
                let duration = t0.elapsed().as_secs_f64() * 1000.0;
                self.log(LogLevel::Debug, stage, "ok", data, Some(duration));
                Ok(result)
            }
            Err(e) => {
                let duration = t0.elapsed().as_secs_f64() * 1000.0;
                let mut err_data = data.unwrap_or_else(|| serde_json::json!({}));
                if let serde_json::Value::Object(ref mut map) = err_data {
                    map.insert("error".to_string(), serde_json::Value::String(e.clone()));
                }
                self.log(LogLevel::Error, stage, &e, Some(err_data), Some(duration));
                Err(e)
            }
        }
    }

    /// Concludes the current trace span, calculates duration, merges optional metadata,
    /// logs the completion, and flushes the trace payload to all registered sinks.
    pub async fn end_trace(&self, meta: Option<serde_json::Value>) -> Option<Trace> {
        let mut trace_opt = self.current_trace.lock().take();
        let instant_opt = self.trace_instant.lock().take();
        
        if let (Some(mut trace), Some(instant)) = (trace_opt.take(), instant_opt) {
            let ended_at = Self::get_now_ms();
            let duration_ms = instant.elapsed().as_secs_f64() * 1000.0;
            
            trace.ended_at = Some(ended_at);
            trace.duration_ms = Some(duration_ms);
            
            if let Some(m) = meta {
                if let Some(ref mut existing_meta) = trace.meta {
                    if let (serde_json::Value::Object(ref mut e_map), serde_json::Value::Object(new_map)) = (existing_meta, m) {
                        for (k, v) in new_map {
                            e_map.insert(k, v);
                        }
                    }
                } else {
                    trace.meta = Some(m);
                }
            }

            self.log(
                LogLevel::Info,
                "trace.end",
                "done",
                Some(serde_json::json!({ "durationMs": format!("{:.0}", duration_ms) })),
                None,
            );

            for sink in &self.sinks {
                let _ = sink.flush_trace(&trace).await;
            }

            Some(trace)
        } else {
            None
        }
    }

    /// Logs a message at Debug severity.
    pub fn debug(&self, msg: &str) {
        self.log(LogLevel::Debug, "general", msg, None, None);
    }

    /// Logs a message at Info severity.
    pub fn info(&self, msg: &str) {
        self.log(LogLevel::Info, "general", msg, None, None);
    }

    /// Logs a message at Warn severity.
    pub fn warn(&self, msg: &str) {
        self.log(LogLevel::Warn, "general", msg, None, None);
    }

    /// Logs a message at Error severity.
    pub fn error(&self, msg: &str) {
        self.log(LogLevel::Error, "general", msg, None, None);
    }
}
