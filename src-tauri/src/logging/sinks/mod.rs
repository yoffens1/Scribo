pub mod noop;
pub mod console;
pub mod memory;
pub mod file;

pub use noop::NoopSink;
pub use console::ConsoleSink;
pub use memory::MemorySink;
pub use file::FileSink;
