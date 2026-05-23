pub mod types;
pub mod logger;
pub mod factory;
pub mod sinks;

pub use types::{LogLevel, LogEvent, Trace, Sink};
pub use logger::Logger;
pub use factory::{LoggerFactory, LoggerFactoryConfig, logger_factory};
pub use sinks::{ConsoleSink, FileSink, MemorySink, NoopSink};

use tracing_subscriber::{fmt, prelude::*, EnvFilter};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};
use tracing_appender::non_blocking::WorkerGuard;

pub fn setup_logger(app: &AppHandle) -> Result<WorkerGuard, Box<dyn std::error::Error>> {
    let app_dir = app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from(".scribo"));
    let log_dir = app_dir.join("logs");
    std::fs::create_dir_all(&log_dir)?;

    let file_appender = tracing_appender::rolling::daily(log_dir, "scribo.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let file_layer = fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(true);

    let console_layer = fmt::layer()
        .with_writer(std::io::stdout)
        .with_ansi(true)
        .with_target(true);

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,scribo_lib=debug"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(file_layer)
        .with(console_layer)
        .try_init()?;

    Ok(guard)
}

#[tauri::command]
pub fn log_event(level: String, namespace: String, message: String, data: Option<serde_json::Value>) {
    let data_str = data.map(|d| d.to_string()).unwrap_or_default();
    match level.as_str() {
        "debug" => tracing::debug!(target: "frontend", namespace = %namespace, data = %data_str, "{}", message),
        "info" => tracing::info!(target: "frontend", namespace = %namespace, data = %data_str, "{}", message),
        "warn" => tracing::warn!(target: "frontend", namespace = %namespace, data = %data_str, "{}", message),
        "error" => tracing::error!(target: "frontend", namespace = %namespace, data = %data_str, "{}", message),
        _ => tracing::info!(target: "frontend", namespace = %namespace, data = %data_str, "{}", message),
    }
}
