pub mod config;
pub mod formatter;
pub mod linter;

pub use formatter::format;
pub use linter::lint;

pub fn is_void_element(name: &str) -> bool {
    matches!(
        name.to_lowercase().as_str(),
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}
