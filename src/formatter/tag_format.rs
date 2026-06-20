//! Rendering a single tag's attribute list: the cached attribute-matching
//! regexes, `format_tag` (wrap / collapse decision + output), and the
//! `style="…"` attribute formatter.
use super::*;

pub(crate) static DJANGO_ATTR_AFTER_RE: OnceLock<Regex> = OnceLock::new();
pub(crate) static DJANGO_ATTR_BEFORE_RE: OnceLock<Regex> = OnceLock::new();
pub(crate) static ATTR_RE: OnceLock<Regex> = OnceLock::new();
pub(crate) static ATTR_RE_BETTER: OnceLock<Regex> = OnceLock::new();

/// The attribute-matching regex used by `format_tag`.  There are only two
/// variants (selected by `better_attribute_parsing`), so each is compiled
/// once and cached rather than rebuilt for every tag in the document.
pub(crate) fn attribute_regex(better_attribute_parsing: bool) -> &'static Regex {
    if better_attribute_parsing {
        ATTR_RE_BETTER.get_or_init(|| {
            Regex::new(
                r#"([a-zA-Z0-9:@._#*!-]+(?:\s*=\s*(?:"(?:\{\{[\s\S]*?\}\}|\{%[\s\S]*?%\}|[^"])*"|'(?:\{\{[\s\S]*?\}\}|\{%[\s\S]*?%\}|[^'])*'|[^\s>]+))?|\{\{[\s\S]*?\}\}|\{%[\s\S]*?%\})"#,
            )
            .unwrap()
        })
    } else {
        ATTR_RE.get_or_init(|| {
            Regex::new(
                r#"([a-zA-Z0-9:@._#*!-]+(?:\s*=\s*(?:"(?:(?:\{%-?\s*(?:if|for|asyncAll|asyncEach)[^\}]*?%\}(?:[\s\S]*?\{%\s*end(?:if|for|each|all)[^\}]*?-?%\})+?)|\{\{[\s\S]*?\}\}|\{%[\s\S]*?%\}|[^"])*"|'(?:(?:\{%-?\s*(?:if|for|asyncAll|asyncEach)[^\}]*?%\}(?:[\s\S]*?\{%\s*end(?:if|for|each|all)[^\}]*?-?%\})+?)|\{\{[\s\S]*?\}\}|\{%[\s\S]*?%\}|[^'])*'|\{\{[\s\S]*?\}\}|[^\s>]+))?|(?:\{%-?\s*(?:if|for|asyncAll|asyncEach)[^\}]*?%\}(?:[\s\S]*?\{%\s*end(?:if|for|each|all)[^\}]*?-?%\})+?)|\{\{[\s\S]*?\}\}|\{%[\s\S]*?%\}|["'])"#,
            )
            .unwrap()
        })
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn format_tag(
    name: &str,
    raw: &str,
    is_self_closing: bool,
    indent_level: usize,
    spaces_before_tag: usize,
    config: &Config,
    is_ignored_block: bool,
    allow_attr_wrap: bool,
) -> String {
    let attr_re = attribute_regex(config.better_attribute_parsing);
    let whitespace_re = WHITESPACE_RE.get_or_init(|| Regex::new(r#"\s+"#).unwrap());

    let start_pos = if raw.starts_with("</") {
        2 + name.len()
    } else {
        1 + name.len()
    };
    let end_pos = if raw.ends_with("/>") {
        raw.len() - 2
    } else {
        raw.len() - 1
    };
    let content = &raw[start_pos..end_pos];

    // Check if the attribute regex fully covers the content. If there are
    // non-whitespace characters between (or after) matches the tag contains
    // syntax the regex cannot parse (e.g. malformed template tags with
    // unbalanced quotes). In that case, preserve the original tag text
    // instead of reformatting, matching djlint's behavior of leaving
    // unparseable tags alone.
    {
        let mut scan_end = 0;
        let mut has_unparsed = false;
        for m in attr_re.find_iter(content) {
            if content[scan_end..m.start()]
                .chars()
                .any(|c| !c.is_ascii_whitespace())
            {
                has_unparsed = true;
                break;
            }
            scan_end = m.end();
        }
        if !has_unparsed
            && content[scan_end..]
                .chars()
                .any(|c| !c.is_ascii_whitespace())
        {
            has_unparsed = true;
        }
        if has_unparsed {
            // Collapse internal whitespace but otherwise preserve as-is.
            let collapsed = whitespace_re.replace_all(raw.trim(), " ");
            return collapsed.to_string();
        }
    }

    // Track per-attribute metadata so the wrapping path can keep attributes
    // inside {% if %}...{% endif %} on the same line and suppress spaces
    // between template tags and their adjacent content (matching djlint).
    let mut attr_depth: Vec<usize> = Vec::new();
    let mut attr_is_django: Vec<bool> = Vec::new();
    // Whether there was whitespace filler between the previous attribute
    // and this one in the original source.  Used by the wrapping path to
    // decide whether to insert a space (e.g. `{% if x %}checked{% endif %}`
    // has no filler, while `{% if x %} a="1" {% endif %}` does).
    let mut attr_has_leading_filler: Vec<bool> = Vec::new();
    let attrs: Vec<String> = {
        let mut depth: usize = 0;
        let mut last_match_end: usize = 0;
        attr_re
            .find_iter(content)
            .map(|m| {
                let filler = &content[last_match_end..m.start()];
                let has_filler = !filler.trim().is_empty()
                    || filler.contains(' ')
                    || filler.contains('\n')
                    || filler.contains('\t');
                attr_has_leading_filler.push(has_filler);
                last_match_end = m.end();

                let raw_attr = m.as_str();
                let is_django_block = raw_attr.starts_with("{%");
                attr_is_django.push(is_django_block);
                if is_django_block {
                    let inner = raw_attr[2..raw_attr.len() - 2].trim();
                    let is_closing = inner.starts_with("end")
                        || inner.starts_with("else")
                        || inner.starts_with("elif");
                    if is_closing {
                        depth = depth.saturating_sub(1);
                    }
                    attr_depth.push(depth);
                    if !is_closing {
                        let is_opener = inner.starts_with("if")
                            || inner.starts_with("for")
                            || inner.starts_with("with")
                            || inner.starts_with("block")
                            || inner.starts_with("filter")
                            || inner.starts_with("autoescape")
                            || inner.starts_with("spaceless");
                        // When the non-better regex matches an entire
                        // {% if %}...{% endif %} block as one token, the
                        // raw attr already contains both opener and closer.
                        // Don't increment depth in that case — the net
                        // change is zero and subsequent attrs should still
                        // get their own wrapped lines.
                        let is_self_contained = raw_attr.contains("endif")
                            || raw_attr.contains("endfor")
                            || raw_attr.contains("endeach")
                            || raw_attr.contains("endall");
                        if is_opener && !is_self_contained {
                            depth += 1;
                        }
                    }
                } else {
                    attr_depth.push(depth);
                }
                let normalized = normalize_django(raw_attr);
                let collapsed = whitespace_re.replace_all(&normalized, " ").to_string();
                // Quote unquoted template-var values: `name={{ ... }}` →
                // `name="{{ ... }}"`.  djlint's format_attributes does this
                // when wrapping (the `attrs` list is only used in the
                // wrapping path, so the non-wrapping output is unaffected).
                if !is_django_block {
                    if let Some(eq_pos) = collapsed.find("={{") {
                        let value_part = collapsed[eq_pos + 1..].trim();
                        if value_part.starts_with("{{") && value_part.ends_with("}}") {
                            format!("{}=\"{}\"", &collapsed[..eq_pos], value_part)
                        } else {
                            collapsed
                        }
                    } else {
                        collapsed
                    }
                } else {
                    collapsed
                }
            })
            .collect()
    };

    let mut last_end = 0;
    let mut prev_was_django_block = false;
    let mut django_block_depth: usize = 0;

    // Build content string without style normalization. djlint only
    // reformats style attributes when wrapping, so this string is used
    // both for the length check and for the non-wrapping output path.
    let mut raw_final_content = String::new();
    for (idx, m) in attr_re.find_iter(content).enumerate() {
        let filler = &content[last_end..m.start()];
        let attr = m.as_str();
        let is_django_block = attr.starts_with("{%") || attr.starts_with("{#");
        let attr_content = if is_django_block && attr.len() > 4 {
            attr[2..attr.len() - 2].trim()
        } else {
            ""
        };
        let is_closing_like = is_django_block
            && (attr_content.starts_with("end")
                || attr_content.starts_with("else")
                || attr_content.starts_with("elif"));

        if !is_ignored_block
            && (prev_was_django_block || (is_closing_like && django_block_depth > 0 && idx > 0))
        {
            raw_final_content.push_str(filler.trim());
        } else {
            raw_final_content.push_str(filler);
        }

        let raw_normalized = normalize_django(attr);
        raw_final_content.push_str(&raw_normalized);

        if is_django_block {
            if attr_content.starts_with("if")
                || attr_content.starts_with("for")
                || attr_content.starts_with("with")
                || attr_content.starts_with("block")
                || attr_content.starts_with("filter")
                || attr_content.starts_with("autoescape")
                || attr_content.starts_with("spaceless")
            {
                django_block_depth += 1;
            } else if attr_content.starts_with("end") {
                django_block_depth = django_block_depth.saturating_sub(1);
            }
        }

        prev_was_django_block = is_django_block;
        last_end = m.end();
    }
    let filler = &content[last_end..];
    if !is_ignored_block && prev_was_django_block {
        raw_final_content.push_str(filler.trim());
    } else {
        raw_final_content.push_str(filler);
    }

    // Collapse whitespace (e.g., multi-line attributes) to single spaces.
    // This string preserves style attribute values as-is (including trailing
    // semicolons), matching djlint which only reformats style when wrapping.
    let raw_attrs_collapsed = whitespace_re.replace_all(&raw_final_content, " ");

    // In non-better parsing mode the entire {% if %}...{% endif %} block is
    // matched as a single attribute token, so spaces between the block
    // delimiters and the enclosed HTML attributes survive whitespace collapse.
    // Remove them here to match djlint's output. Skip for ignored blocks
    // (textarea, pre) where attribute whitespace is left as-is.
    let raw_attrs_collapsed = if !is_ignored_block {
        let after_re =
            DJANGO_ATTR_AFTER_RE.get_or_init(|| Regex::new(r"(%\}) ([a-zA-Z0-9_])").unwrap());
        // Only remove space before closing blocks ({% end... %}, {% else %}, {% elif %})
        // to avoid stripping the space before opening blocks like {% if a %} in
        // `attr="v" {% if a %}`.
        let before_re = DJANGO_ATTR_BEFORE_RE
            .get_or_init(|| Regex::new(r#"([a-zA-Z0-9_"']) (\{%-?\s*(?:end|else|elif))"#).unwrap());
        let s = after_re.replace_all(&raw_attrs_collapsed, "$1$2");
        before_re.replace_all(&s, "$1$2").into_owned().into()
    } else {
        raw_attrs_collapsed
    };

    let raw_attrs_len = raw_attrs_collapsed.trim().len();

    // djlint's expand step breaks block template tags (`{% if %}` …
    // `{% endif %}`) onto their own indented lines even inside an attribute
    // value, when the value contains a bare `{`/`}` that is not part of a
    // template tag (e.g. an Alpine/JS object literal `x-data="{ … }"`). The
    // bare brace defeats djlint's "is this template tag inside an html tag"
    // back-scan, so the tag is treated as loose. Such tags also escape
    // attribute wrapping, so when expansion applies we emit the tag inline
    // (non-wrapped) with the rewritten, multi-line attribute value.
    if !is_ignored_block {
        let inner_indent = " ".repeat((indent_level + 1) * config.indent);
        if let Some(expanded) =
            expand_attr_template_blocks(&raw_attrs_collapsed, &inner_indent, config)
        {
            let mut formatted = if raw.starts_with("</") {
                format!("</{}", name)
            } else {
                format!("<{}", name)
            };
            formatted.push_str(expanded.trim_end());
            if raw.ends_with("/>") || (is_self_closing && config.close_void_tags) {
                formatted.push_str(" />");
            } else {
                formatted.push('>');
            }
            return formatted;
        }
    }

    let total_line_len =
        (indent_level * config.indent) + name.len() + 1 + raw_attrs_collapsed.len() + 1;

    let allow_wrap = should_wrap_attributes(name) || allow_attr_wrap;
    if (raw_attrs_len < config.max_attribute_length && total_line_len <= config.max_line_length)
        || !allow_wrap
    {
        // When not wrapping, use raw_attrs_collapsed which preserves the
        // original style attribute (including trailing semicolons), matching
        // djlint's behavior of only reformatting style when wrapping.
        let mut formatted = if raw.starts_with("</") {
            format!("</{}", name)
        } else {
            format!("<{}", name)
        };

        formatted.push_str(raw_attrs_collapsed.trim_end());

        if raw.ends_with("/>") || (is_self_closing && config.close_void_tags) {
            formatted.push_str(" />");
        } else {
            formatted.push('>');
        }
        return formatted;
    }

    let mut formatted = format!("<{}", name);
    // djlint computes attr_indent as (whitespace immediately before the `<`)
    // + len("<tagname ").  `spaces_before_tag` is that leading whitespace
    // count, computed by the caller from the current output state.
    let attr_indent = " ".repeat(spaces_before_tag + name.len() + 2);

    for (idx, attr) in attrs.iter().enumerate() {
        if idx == 0 {
            formatted.push(' ');
        } else if attr_depth[idx] > 0 || attr_depth[idx.saturating_sub(1)] > attr_depth[idx] {
            // Inside a django block or closing a django block: stay on the
            // same line.  Only add a space if the original source had
            // whitespace filler between these attributes (e.g.
            // `{% if x %} a="1" {% endif %}` has filler, but
            // `{% if x %}checked{% endif %}` does not).
            if attr_has_leading_filler[idx] {
                formatted.push(' ');
            }
        } else {
            formatted.push('\n');
            formatted.push_str(&attr_indent);
        }
        if attr.starts_with("style=") {
            formatted.push_str(&format_style_attribute(attr, &attr_indent));
        } else {
            formatted.push_str(attr);
        }
    }

    if raw.ends_with("/>") || (is_self_closing && config.close_void_tags) {
        formatted.push_str(" />");
    } else {
        formatted.push('>');
    }

    formatted
}

/// Rewrite an attribute string, expanding any quoted value that contains a
/// "loose" block template tag onto its own indented lines (see the call site
/// in `format_tag` for the djlint behaviour this mirrors). Returns `None` when
/// no value qualifies, so callers can fall through to the normal path.
fn expand_attr_template_blocks(
    attrs: &str,
    inner_indent: &str,
    config: &Config,
) -> Option<String> {
    let mut result = String::new();
    let mut changed = false;
    let mut rest = attrs;
    loop {
        match rest.find(['"', '\'']) {
            None => {
                result.push_str(rest);
                break;
            }
            Some(qpos) => {
                let quote = rest.as_bytes()[qpos] as char;
                result.push_str(&rest[..qpos]);
                let after_q = &rest[qpos + 1..];
                match after_q.find(quote) {
                    None => {
                        // Unterminated quote: copy the remainder verbatim.
                        result.push_str(&rest[qpos..]);
                        break;
                    }
                    Some(endrel) => {
                        let value = &after_q[..endrel];
                        result.push(quote);
                        if let Some(expanded) = expand_attr_value(value, inner_indent, config) {
                            result.push_str(&expanded);
                            changed = true;
                        } else {
                            result.push_str(value);
                        }
                        result.push(quote);
                        rest = &after_q[endrel + 1..];
                    }
                }
            }
        }
    }
    if changed {
        Some(result)
    } else {
        None
    }
}

/// Expand the block template tags inside a single attribute value, but only
/// when the value contains a bare `{`/`}` (not part of a template tag). A
/// line break + `inner_indent` is inserted before each block opener and after
/// each block closer. Returns `None` when no expansion applies.
fn expand_attr_value(value: &str, inner_indent: &str, config: &Config) -> Option<String> {
    if !value_has_bare_brace(value) {
        return None;
    }
    let mut out = String::new();
    let mut any_block = false;
    let mut rest = value;
    while let Some(pos) = rest.find("{%") {
        let Some(endrel) = rest[pos..].find("%}") else {
            break;
        };
        let tag_end = pos + endrel + 2;
        let tag = &rest[pos..tag_end];
        let inner = tag[2..tag.len() - 2].trim();
        let first_word = inner.split_whitespace().next().unwrap_or("");
        let is_closer = first_word.starts_with("end");
        let is_block = crate::tags::is_django_block_tag(first_word, &config.custom_blocks);

        out.push_str(&rest[..pos]);
        if is_block && !is_closer {
            out.push('\n');
            out.push_str(inner_indent);
            out.push_str(tag);
            any_block = true;
        } else if is_block && is_closer {
            out.push_str(tag);
            out.push('\n');
            out.push_str(inner_indent);
            any_block = true;
        } else {
            out.push_str(tag);
        }
        rest = &rest[tag_end..];
    }
    out.push_str(rest);
    if any_block {
        Some(out)
    } else {
        None
    }
}

/// Whether `value` contains a `{` or `}` that is not part of a `{{ }}`,
/// `{% %}`, or `{# #}` template tag.
fn value_has_bare_brace(value: &str) -> bool {
    let mut rest = value;
    loop {
        match rest.find(['{', '}']) {
            None => return false,
            Some(pos) => {
                let b = &rest[pos..];
                if b.starts_with("{{") || b.starts_with("{%") || b.starts_with("{#") {
                    let close = if b.starts_with("{{") {
                        "}}"
                    } else if b.starts_with("{%") {
                        "%}"
                    } else {
                        "#}"
                    };
                    match b.find(close) {
                        Some(e) => rest = &b[e + 2..],
                        None => return true,
                    }
                } else if b.starts_with("}}") || b.starts_with("%}") || b.starts_with("#}") {
                    rest = &b[2..];
                } else {
                    return true;
                }
            }
        }
    }
}

pub(crate) fn format_style_attribute(attr: &str, indent: &str) -> String {
    // Expect style="prop1: val1; prop2: val2;"
    let quote = if attr.contains("=\"") { "\"" } else { "'" };
    let content_start = attr.find(quote).unwrap_or(0) + 1;
    let content_end = attr.rfind(quote).unwrap_or(attr.len());
    let content = &attr[content_start..content_end];

    let props: Vec<String> = content
        .split(';')
        .map(|s| normalize_django(s.trim()))
        .filter(|s| !s.is_empty())
        .collect();

    if props.is_empty() {
        return attr.to_string();
    }

    if indent.is_empty() {
        // No wrapping requested, but still strip trailing semicolon if it's the only property
        if props.len() == 1 {
            return format!("style={}{}{}", quote, props[0], quote);
        }
        let mut result = format!("style={}{}", quote, props[0]);
        for prop in props.iter().skip(1) {
            result.push_str("; ");
            result.push_str(prop);
        }
        result.push_str(quote);
        return result;
    }

    let mut result = format!("style={}{}", quote, props[0]);
    if props.len() > 1 {
        for prop in props.iter().skip(1) {
            result.push(';');
            result.push('\n');
            result.push_str(indent);
            // Add additional indent for style property
            result.push_str("       "); // "style=\"" is 7 chars
            result.push_str(prop);
        }
    }
    // djlint seems to strip trailing semicolon when wrapping
    result.push_str(quote);
    result
}
