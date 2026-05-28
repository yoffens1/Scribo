//! # Logger Factory
//!
//! Provides a configurable factory for creating `Logger` instances with uniform sink sets
//! and logging levels, supporting dynamic reconfiguration.

use crate::logging::types::{LogLevel, Sink};
use crate::logging::logger::Logger;
use crate::logging::sinks::ConsoleSink;
use std::sync::{Arc, OnceLock};
use parking_lot::RwLock;

/// Configuration payload for the logger factory.
pub struct LoggerFactoryConfig {
    /// Toggle to enable or disable log capture globally.
    pub enabled: bool,
    /// Minimum severity level required for events to be logged.
    pub min_level: LogLevel,
    /// List of output destinations where events and traces are written.
    pub sinks: Vec<Arc<dyn Sink>>,
}

/// Dynamic factory that spawns namespaced `Logger` instances.
pub struct LoggerFactory {
    config: RwLock<LoggerFactoryConfig>,
}

impl LoggerFactory {
    /// Creates a new `LoggerFactory` with the given configuration.
    pub fn new(config: LoggerFactoryConfig) -> Self {
        Self {
            config: RwLock::new(config),
        }
    }
    
    /// Spawns a new namespaced `Logger` using the current factory settings.
    pub fn create(&self, namespace: &str) -> Logger {
        let conf = self.config.read();
        Logger::new(
            namespace.to_string(),
            conf.enabled,
            conf.min_level,
            conf.sinks.clone(),
        )
    }
    
    /// Reconfigures the factory parameters dynamically. All subsequently created loggers
    /// will inherit these settings, while existing loggers retain their initialized configs.
    pub fn reconfigure(&self, enabled: Option<bool>, min_level: Option<LogLevel>, sinks: Option<Vec<Arc<dyn Sink>>>) {
        let mut conf = self.config.write();
        if let Some(e) = enabled {
            conf.enabled = e;
        }
        if let Some(lvl) = min_level {
            conf.min_level = lvl;
        }
        if let Some(s) = sinks {
            conf.sinks = s;
        }
    }
}

/// Retrieves the static, global singleton `LoggerFactory` instance.
pub fn logger_factory() -> &'static LoggerFactory {
    static FACTORY: OnceLock<LoggerFactory> = OnceLock::new();
    FACTORY.get_or_init(|| {
        LoggerFactory::new(LoggerFactoryConfig {
            enabled: true,
            min_level: LogLevel::Debug,
            sinks: vec![Arc::new(ConsoleSink)],
        })
    })
}
