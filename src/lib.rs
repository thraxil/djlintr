pub mod config;
pub mod formatter;
pub mod linter;
pub mod tags;

pub use formatter::format;
pub use linter::lint;
pub use tags::is_void_element;
