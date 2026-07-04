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

/// HTML tags that djlint's expand step breaks onto their own lines
/// (the `break_html_tags` setting). Tags NOT in this list (e.g.
/// `canvas`, `noscript`, `address`, `article`) are never split by
/// expand, so an empty element's closing tag stays inline with its
/// opening tag. Void/self-closing elements are included via
/// `is_void_element`.
pub fn is_break_html_tag(name: &str) -> bool {
    if is_void_element(name) {
        return true;
    }
    matches!(
        name.to_lowercase().as_str(),
        "html"
            | "head"
            | "body"
            | "div"
            | "nav"
            | "ul"
            | "ol"
            | "dl"
            | "dd"
            | "dt"
            | "li"
            | "table"
            | "thead"
            | "tbody"
            | "tr"
            | "th"
            | "td"
            | "blockquote"
            | "select"
            | "form"
            | "option"
            | "optgroup"
            | "fieldset"
            | "legend"
            | "label"
            | "header"
            | "cache"
            | "main"
            | "section"
            | "aside"
            | "footer"
            | "figure"
            | "figcaption"
            | "video"
            | "p"
            | "g"
            | "svg"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "button"
            | "path"
            | "picture"
            | "script"
            | "style"
            | "details"
            | "summary"
    )
}

/// Tags whose content djlint treats as verbatim / an ignored block
/// (`<script>`, `<style>`, `<pre>`, `<textarea>`). djlint does not normalise
/// template-tag spacing (`{{x}}` → `{{ x }}`) on these — including in their
/// opening tag's attributes.
pub fn is_verbatim_tag(name: &str) -> bool {
    matches!(
        name.to_lowercase().as_str(),
        "script" | "style" | "pre" | "textarea"
    )
}

/// Whether djlint's expand step forces this tag onto its own line — i.e. it is
/// a block tag that is also in `break_html_tags`. Tags that are neither (SVG
/// shapes like `<circle>`, verbatim `<pre>`/`<textarea>`) or block-but-not-broken
/// (`<canvas>`, `<noscript>`) stay on the same line as their neighbours, so
/// they behave "inline-ish" for line-break decisions.
pub fn breaks_onto_own_line(name: &str) -> bool {
    is_html_block_tag(name) && is_break_html_tag(name)
}

/// HTML tags djlint treats as indentable (`indent_html_tags`). djlint's
/// expand step only leaves a block template tag (`{% if %}` …) un-broken
/// inside an attribute value when the value's enclosing tag is one of these
/// (and the back-scan to it isn't blocked). Tags NOT in this list — e.g. SVG
/// shapes `rect`/`path`/`circle` — cause such template tags to be broken onto
/// their own lines.
pub fn is_indent_html_tag(name: &str) -> bool {
    matches!(
        name.to_lowercase().as_str(),
        "a" | "abbr"
            | "acronym"
            | "address"
            | "applet"
            | "area"
            | "article"
            | "aside"
            | "audio"
            | "b"
            | "base"
            | "basefont"
            | "bdi"
            | "bdo"
            | "bgsound"
            | "big"
            | "blink"
            | "blockquote"
            | "body"
            | "br"
            | "button"
            | "canvas"
            | "caption"
            | "center"
            | "cite"
            | "code"
            | "col"
            | "colgroup"
            | "command"
            | "content"
            | "data"
            | "datalist"
            | "dd"
            | "del"
            | "details"
            | "dfn"
            | "dialog"
            | "dir"
            | "div"
            | "dl"
            | "dt"
            | "element"
            | "em"
            | "embed"
            | "fieldset"
            | "figcaption"
            | "figure"
            | "font"
            | "footer"
            | "form"
            | "frame"
            | "frameset"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "head"
            | "header"
            | "hgroup"
            | "hr"
            | "html"
            | "i"
            | "iframe"
            | "image"
            | "img"
            | "input"
            | "ins"
            | "isindex"
            | "kbd"
            | "keygen"
            | "label"
            | "legend"
            | "li"
            | "link"
            | "listing"
            | "main"
            | "map"
            | "mark"
            | "marquee"
            | "math"
            | "menu"
            | "menuitem"
            | "meta"
            | "meter"
            | "multicol"
            | "nav"
            | "nextid"
            | "nobr"
            | "noembed"
            | "noframes"
            | "noscript"
            | "object"
            | "ol"
            | "optgroup"
            | "option"
            | "output"
            | "p"
            | "param"
            | "picture"
            | "plaintext"
            | "pre"
            | "progress"
            | "q"
            | "rb"
            | "rbc"
            | "rp"
            | "rt"
            | "rtc"
            | "ruby"
            | "s"
            | "samp"
            | "script"
            | "section"
            | "select"
            | "shadow"
            | "slot"
            | "small"
            | "source"
            | "spacer"
            | "span"
            | "strike"
            | "strong"
            | "style"
            | "sub"
            | "summary"
            | "sup"
            | "svg"
            | "table"
            | "tbody"
            | "td"
            | "template"
            | "textarea"
            | "tfoot"
            | "th"
            | "thead"
            | "time"
            | "title"
            | "tr"
            | "track"
            | "tt"
            | "u"
            | "ul"
            | "var"
            | "video"
            | "wbr"
            | "xmp"
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
