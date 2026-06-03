//! Centralized HTML tag and attribute classification.
//!
//! All tag-name lookup functions perform case-insensitive matching by
//! lowercasing the input before comparison.

/// HTML void elements (self-closing, no end tag).
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

/// Inline HTML elements that can appear mid-line without forcing a
/// line break in the formatter.
pub fn is_inline_tag(name: &str) -> bool {
    matches!(
        name.to_lowercase().as_str(),
        "a" | "abbr"
            | "acronym"
            | "b"
            | "bdo"
            | "big"
            | "cite"
            | "code"
            | "dfn"
            | "em"
            | "i"
            | "kbd"
            | "map"
            | "object"
            | "q"
            | "samp"
            | "small"
            | "span"
            | "strong"
            | "sub"
            | "sup"
            | "tt"
            | "var"
            | "title"
            | "option"
            | "script"
            | "style"
            | "time"
    )
}

/// Block-level HTML elements that get their own lines in formatted output.
pub fn is_html_block_tag(name: &str) -> bool {
    if is_void_element(name) {
        return true;
    }
    matches!(
        name.to_lowercase().as_str(),
        "address"
            | "article"
            | "aside"
            | "blockquote"
            | "body"
            | "canvas"
            | "details"
            | "dd"
            | "div"
            | "dl"
            | "dt"
            | "fieldset"
            | "figcaption"
            | "figure"
            | "footer"
            | "form"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "head"
            | "header"
            | "hr"
            | "html"
            | "li"
            | "main"
            | "nav"
            | "noscript"
            | "ol"
            | "p"
            | "pre"
            | "section"
            | "table"
            | "tbody"
            | "td"
            | "tfoot"
            | "th"
            | "thead"
            | "tr"
            | "ul"
            | "video"
            | "svg"
            | "g"
            | "defs"
            | "clippath"
            | "mask"
            | "pattern"
            | "lineargradient"
            | "radialgradient"
            | "stop"
            | "text"
            | "tspan"
    )
}

/// Tags that may be condensed onto a single line when their content
/// is short enough. Mirrors the Python djlint
/// `optional_single_line_html_tags` list.
pub fn is_condensable_tag(name: &str) -> bool {
    matches!(
        name.to_lowercase().as_str(),
        "a" | "article"
            | "b"
            | "body"
            | "button"
            | "div"
            | "dt"
            | "em"
            | "figcaption"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "head"
            | "icon"
            | "label"
            | "legend"
            | "li"
            | "link"
            | "option"
            | "p"
            | "path"
            | "script"
            | "select"
            | "small"
            | "span"
            | "strong"
            | "style"
            | "summary"
            | "td"
            | "th"
            | "title"
            | "tr"
    )
}

/// Structural container tags whose children always get their own
/// lines (never collapsed inline).
pub fn is_structural_tag(name: &str) -> bool {
    matches!(
        name.to_lowercase().as_str(),
        "table" | "tbody" | "thead" | "tfoot" | "ul" | "ol" | "dl" | "form" | "dd"
    )
}

/// Returns `false` for SVG grouping elements and verbatim tags
/// (script/style) whose children should NOT be indented.
pub fn should_indent_children(name: &str) -> bool {
    !matches!(
        name.to_lowercase().as_str(),
        "g" | "defs"
            | "clippath"
            | "mask"
            | "pattern"
            | "lineargradient"
            | "radialgradient"
            | "symbol"
            | "marker"
            | "script"
            | "style"
    )
}

/// Returns `false` for SVG shape elements whose attributes should
/// NOT be wrapped.
pub fn should_wrap_attributes(name: &str) -> bool {
    !matches!(
        name.to_lowercase().as_str(),
        "path"
            | "circle"
            | "rect"
            | "line"
            | "polyline"
            | "polygon"
            | "ellipse"
            | "textarea"
            | "pre"
            | "g"
            | "defs"
            | "clippath"
            | "mask"
            | "pattern"
            | "lineargradient"
            | "radialgradient"
            | "stop"
            | "text"
            | "tspan"
    )
}

/// Tag names that trigger H009 ("tag names should be lowercase")
/// when they appear in UPPERCASE in the source.
pub const H009_TAGS: &[&str] = &[
    "HTML",
    "BODY",
    "DIV",
    "P",
    "SPAN",
    "TABLE",
    "TR",
    "TD",
    "TH",
    "THEAD",
    "TBODY",
    "CODE",
    "UL",
    "OL",
    "LI",
    "H1",
    "H2",
    "H3",
    "H4",
    "H5",
    "H6",
    "A",
    "DD",
    "DT",
    "BLOCKQUOTE",
    "SELECT",
    "FORM",
    "FIELDSET",
    "OPTGROUP",
    "LEGEND",
    "LABEL",
    "HEADER",
    "CACHE",
    "MAIN",
    "ASIDE",
    "FOOTER",
    "SECTION",
    "NAME",
    "FIGURE",
    "FIGCAPTION",
    "VIDEO",
    "G",
    "SVG",
    "BUTTON",
    "PATH",
    "PICTURE",
    "SCRIPT",
    "STYLE",
    "DETAILS",
    "SUMMARY",
];

/// Attribute names that trigger H010 ("attribute names should be
/// lowercase") when they appear in UPPERCASE in the source.
pub const H010_ATTRS: &[&str] = &[
    "CLASS", "ID", "SRC", "WIDTH", "HEIGHT", "ALT", "STYLE", "LANG", "TITLE", "MEDIA", "SRCSET",
];

/// Django/Jinja block-level template tags that affect indentation.
pub fn is_django_block_tag(name: &str, custom_blocks: &[String]) -> bool {
    let name_lower = name.to_lowercase();
    let actual_name = name_lower.strip_prefix("end").unwrap_or(&name_lower);

    matches!(
        actual_name,
        "block"
            | "if"
            | "ifchanged"
            | "for"
            | "with"
            | "autoescape"
            | "filter"
            | "spaceless"
            | "cache"
            | "macro"
            | "call"
            | "set"
            | "localize"
            | "compress"
            | "comment"
            | "load"
            | "extends"
    ) || custom_blocks
        .iter()
        .any(|b| b.to_lowercase() == actual_name)
}
