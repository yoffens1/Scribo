use crate::logging::types::{LogLevel, Sink};
use crate::logging::logger::Logger;
use crate::logging::sinks::ConsoleSink;
use std::sync::{Arc, OnceLock};
use parking_lot::RwLock;

pub struct LoggerFactoryConfig {
    pub enabled: bool,
    pub min_level: LogLevel,
    pub sinks: Vec<Arc<dyn Sink>>,
}

pub struct LoggerFactory {
    config: RwLock<LoggerFactoryConfig>,
}

impl LoggerFactory {
    pub fn new(config: LoggerFactoryConfig) -> Self {
        Self {
            config: RwLock::new(config),
        }
    }
    
    pub fn create(&self, namespace: &str) -> Logger {
        let conf = self.config.read();
        Logger::new(
            namespace.to_string(),
            conf.enabled,
            conf.min_level,
            conf.sinks.clone(),
        )
    }
    
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
