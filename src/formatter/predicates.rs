//! Small, pure classification helpers: tag-name extraction and the
//! inline / block / reindent / line-break predicates the formatter
//! consults while walking tokens.
use super::*;

pub(crate) fn get_django_tag_name(raw: &str) -> Option<&str> {
    let mut bytes = raw.as_bytes();
    if bytes.starts_with(b"{%") {
        bytes = &bytes[2..];
    }
    if bytes.starts_with(b"-") {
        bytes = &bytes[1..];
    }

    // Find start of word
    let mut start = 0;
    while start < bytes.len() && bytes[start].is_ascii_whitespace() {
        start += 1;
    }

    if start == bytes.len() {
        return None;
    }

    // Find end of word
    let mut end = start;
    while end < bytes.len()
        && !bytes[end].is_ascii_whitespace()
        && bytes[end] != b'%'
        && bytes[end] != b'-'
    {
        end += 1;
    }

    // SAFETY: We only advanced indices over ASCII characters (whitespace, '%', '-'),
    // so `start` and `end` are valid UTF-8 boundaries within `raw`.
    let start_idx = raw.len() - bytes.len() + start;
    let end_idx = raw.len() - bytes.len() + end;

    Some(&raw[start_idx..end_idx])
}

/// Strip the "end" prefix from a Django tag name (e.g., "endblock" -> "block").
/// Returns the original name if it doesn't start with "end".
pub(crate) fn strip_end_prefix(name: &str) -> &str {
    name.strip_prefix("end").unwrap_or(name)
}

pub(crate) fn is_reindent_tag(name: &str) -> bool {
    matches!(name, "else" | "elif" | "empty")
}

pub(crate) fn is_strictly_inline(token: &Token, config: &Config, is_parent_django: bool) -> bool {
    match token {
        Token::DjangoVar { .. } => true,
        Token::DjangoBlock { raw, .. } => {
            if is_parent_django {
                return false;
            }
            let tag_name = get_django_tag_name(raw).unwrap_or("");
            if is_reindent_tag(tag_name) {
                return false;
            }
            if is_django_block_self_closing(raw) {
                return true;
            }
            !is_block_tag(strip_end_prefix(tag_name), &config.custom_blocks)
        }
        Token::Text { raw, .. } => !raw.trim().contains('\n'),
        Token::Tag { name, raw, .. } => {
            if is_parent_django {
                is_inline_tag(name) && !raw.contains("{%")
            } else {
                is_inline_tag(name) && !raw.contains("{%") && !raw.contains("{{")
            }
        }
        Token::Comment { raw, .. } | Token::DjangoComment { raw, .. } => !raw.contains('\n'),
        Token::Doctype { .. } => false,
    }
}

pub(crate) fn is_inline_ish(token: &Token, config: &Config) -> bool {
    match token {
        Token::DjangoVar { .. } => true,
        Token::DjangoBlock { raw, .. } => {
            let tag_name = get_django_tag_name(raw).unwrap_or("");
            if is_reindent_tag(tag_name) {
                return false;
            }
            if is_line_break_tag(tag_name) {
                return false;
            }
            !is_block_tag(strip_end_prefix(tag_name), &config.custom_blocks)
        }
        Token::Text { raw, .. } => !raw.starts_with('\n') && !raw.starts_with("\r\n"),
        Token::Tag { name, .. } => is_inline_tag(name),
        Token::Comment { raw, .. } | Token::DjangoComment { raw, .. } => !raw.contains('\n'),
        Token::Doctype { .. } => false,
    }
}

pub(crate) fn is_block_tag(name: &str, custom_blocks: &[String]) -> bool {
    crate::tags::is_django_block_tag(name, custom_blocks)
}

/// Returns true when the source line starting at `from_pos` ends with a
/// net-negative inline-tag balance, meaning there is an unmatched inline
/// closing tag at the end of the line.  Used to pre-decrement indent before
/// emitting content on that line, matching djlint's item-level behaviour.
pub(crate) fn line_ends_with_net_inline_close(tokens: &[Token], from_pos: usize) -> bool {
    if from_pos >= tokens.len() {
        return false;
    }
    let line = tokens[from_pos].line();
    let mut net: i32 = 0;
    let mut last_was_inline_close = false;
    let mut j = from_pos;
    while j < tokens.len() && tokens[j].line() == line {
        match &tokens[j] {
            Token::Tag {
                name,
                is_closing: false,
                is_self_closing: false,
                ..
            } if is_inline_tag(name) => {
                net += 1;
                last_was_inline_close = false;
            }
            Token::Tag {
                name,
                is_closing: true,
                ..
            } if is_inline_tag(name) && !is_break_before_close_tag(name) => {
                net -= 1;
                last_was_inline_close = true;
            }
            Token::Tag { .. } => {
                last_was_inline_close = false;
            }
            _ => {}
        }
        j += 1;
    }
    last_was_inline_close && net < 0
}

/// Tags that djlint's `expand_html` step always places on their own line
/// (break before AND after) even though they appear in `is_inline_tag` for
/// collapse purposes.  When one of these appears as a CLOSER after non-
/// whitespace content on the same source line, it must be forced onto its
/// own line (matching djlint's `break_html_tags` behaviour).
pub(crate) fn is_break_before_close_tag(name: &str) -> bool {
    matches!(name.to_lowercase().as_str(), "option")
}

/// Template tags that always occupy their own line but do NOT open an
/// indented block (no matching end-tag).  They are still collapsible by
/// `try_collapse_html_tag` when they are the sole content of a short
/// parent — matching djlint's two-phase expand-then-condense behaviour.
pub(crate) fn is_line_break_tag(name: &str) -> bool {
    matches!(name, "include")
}

/// Returns true for Cotton-style self-closing block tags (ending with `/ %}`).
/// These never open an indented block even when the tag name is in custom_blocks.
pub(crate) fn is_django_block_self_closing(raw: &str) -> bool {
    let s = raw
        .trim_end_matches('}')
        .trim_end_matches('%')
        .trim_end_matches('-')
        .trim_end_matches(' ');
    s.ends_with('/')
}

pub(crate) fn can_have_closing_tag(name: &str, custom_blocks: &[String]) -> bool {
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
            | "verbatim"
            | "language"
            | "thumbnail"
            | "raw"
    ) || custom_blocks
        .iter()
        .any(|b| b.eq_ignore_ascii_case(actual_name))
}
