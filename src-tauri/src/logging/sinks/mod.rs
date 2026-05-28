//! # Logging Sinks
//!
//! Exposes built-in log output destinations including the terminal console,
//! rolling files, local memory vectors, and a no-op discard sink.

pub mod noop;
pub mod console;
pub mod memory;
pub mod file;

pub use noop::NoopSink;
pub use console::ConsoleSink;
pub use memory::MemorySink;
pub use file::FileSink;
