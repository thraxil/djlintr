//! Rendering a single tag's attribute list: the cached attribute-matching
//! regexes, `format_tag` (wrap / collapse decision + output), and the
//! `style="…"` attribute formatter.
use super::*;

pub(crate) static DJANGO_ATTR_AFTER_RE: OnceLock<Regex> = OnceLock::new();
pub(crate) static DJANGO_ATTR_BEFORE_RE: OnceLock<Regex> = OnceLock::new();
pub(crate) static ATTR_RE: OnceLock<Regex> = OnceLock::new();
pub(crate) static ATTR_RE_BETTER: OnceLock<Regex> = OnceLock::new();
/// A whitespace run that contains at least one newline. djlint's compress
/// step folds such runs (newline + indentation) to a single space, but leaves
/// pure space/tab runs inside attribute values untouched — so `class="a  b"`
/// keeps its double space while `class="a\n  b"` becomes `class="a b"`.
static ATTR_NEWLINE_WS_RE: OnceLock<Regex> = OnceLock::new();

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
    // djlint does not normalise template-tag spacing (`{{x}}` → `{{ x }}`) in
    // the attributes of verbatim tags (<script>/<style>/<pre>/<textarea>), so
    // their attribute values are kept as-is.
    let is_verbatim = crate::tags::is_verbatim_tag(name);
    let attr_re = attribute_regex(config.better_attribute_parsing);
    let whitespace_re = WHITESPACE_RE.get_or_init(|| Regex::new(r#"\s+"#).unwrap());
    // Used for attribute content: collapse only newline-containing runs (see
    // `ATTR_NEWLINE_WS_RE`), preserving author-intended double spaces.
    let attr_ws_re = ATTR_NEWLINE_WS_RE.get_or_init(|| Regex::new(r#"\s*[\r\n]\s*"#).unwrap());

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
                let normalized = if is_verbatim {
                    raw_attr.to_string()
                } else {
                    normalize_django(raw_attr)
                };
                let collapsed = attr_ws_re.replace_all(&normalized, " ").to_string();
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

        let raw_normalized = if is_verbatim {
            attr.to_string()
        } else {
            normalize_django(attr)
        };
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

    // Fold newline-containing whitespace runs (e.g. multi-line attributes) to
    // single spaces, but preserve author-intended double spaces within values
    // (djlint only naturalises line breaks). This string preserves style
    // attribute values as-is (including trailing semicolons), matching djlint
    // which only reformats style when wrapping.
    let raw_attrs_collapsed = attr_ws_re.replace_all(&raw_final_content, " ");

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
    // `{% endif %}`) onto their own indented lines inside an attribute value
    // when the value's enclosing tag is not in `indent_html_tags` (e.g. an SVG
    // `<rect>`), or when the value contains a bare `{`/`}` that is not part of
    // a template tag (e.g. an Alpine/JS object literal `x-data="{ … }"`). Both
    // defeat djlint's "is this template tag inside an html tag" back-scan, so
    // the tag is treated as loose. Such tags also escape attribute wrapping,
    // so when expansion applies we emit the tag inline (non-wrapped) with the
    // rewritten, multi-line attribute value.
    if !is_ignored_block {
        let tag_in_indent = crate::tags::is_indent_html_tag(name);
        // Lines inside the value sit one level deeper than the tag only when
        // the tag itself increments indentation (it is in indent_html_tags).
        let base_level = indent_level + usize::from(tag_in_indent);
        if let Some(expanded) =
            expand_attr_template_blocks(&raw_attrs_collapsed, base_level, !tag_in_indent, config)
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
/// in `format_tag` for the djlint behaviour this mirrors). `base_level` is the
/// indent level for the value's top-level lines; `always` forces expansion
/// even without a bare brace (used when the enclosing tag is not in
/// `indent_html_tags`). Returns `None` when no value qualifies.
fn expand_attr_template_blocks(
    attrs: &str,
    base_level: usize,
    always: bool,
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
                match find_attr_value_end(after_q, quote) {
                    None => {
                        // Unterminated quote: copy the remainder verbatim.
                        result.push_str(&rest[qpos..]);
                        break;
                    }
                    Some(endrel) => {
                        let value = &after_q[..endrel];
                        result.push(quote);
                        if let Some(expanded) = expand_attr_value(value, base_level, always, config)
                        {
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

/// Find the byte offset of the closing `quote` for an attribute value,
/// skipping over `{{ … }}`/`{% … %}` template tags that may themselves contain
/// the quote character (e.g. `class="{% if x == "home" %}…"`). Returns `None`
/// if no closing quote is found.
fn find_attr_value_end(s: &str, quote: char) -> Option<usize> {
    let mut i = 0;
    while i < s.len() {
        let rest = &s[i..];
        if rest.starts_with(quote) {
            return Some(i);
        }
        if rest.starts_with("{%") {
            let e = rest.find("%}")?;
            i += e + 2;
            continue;
        }
        if rest.starts_with("{{") {
            let e = rest.find("}}")?;
            i += e + 2;
            continue;
        }
        i += rest.chars().next().map_or(1, char::len_utf8);
    }
    None
}

/// One rendered line of an expanded attribute value: its indent `level` and
/// its (already-trimmed) text content.
struct AttrLine {
    level: usize,
    text: String,
}

/// Expand the block template tags inside a single attribute value. Mirrors
/// djlint's expand → indent → condense for the value: each block tag goes on
/// its own line (openers increment indent, closers decrement), text between
/// tags is indented one level deeper, and a simple `{% if %}…{% endif %}`
/// block with a single text child and no `{% else %}`/nested tags is rejoined
/// onto one line. Returns `None` when no block template tag qualifies.
fn expand_attr_value(
    value: &str,
    base_level: usize,
    always: bool,
    config: &Config,
) -> Option<String> {
    if !always && !backscan_blocked(value, &config.custom_blocks) {
        return None;
    }

    // The leading text (before the first block tag) stays on the tag's line.
    let mut head = String::new();
    let mut head_done = false;
    let mut lines: Vec<AttrLine> = Vec::new();
    let mut level = base_level;
    let mut any_block = false;
    // Text accumulated since the last block tag (or value start).
    let mut pending = String::new();
    let mut rest = value;

    let flush_pending = |pending: &mut String, lines: &mut Vec<AttrLine>, level: usize| {
        let t = pending.trim();
        if !t.is_empty() {
            lines.push(AttrLine {
                level,
                text: t.to_string(),
            });
        }
        pending.clear();
    };

    while let Some(pos) = rest.find("{%") {
        let Some(endrel) = rest[pos..].find("%}") else {
            break;
        };
        let tag_end = pos + endrel + 2;
        let tag = &rest[pos..tag_end];
        let inner = tag[2..tag.len() - 2].trim();
        let first_word = inner.split_whitespace().next().unwrap_or("");
        let is_closer = first_word.starts_with("end");
        let is_middle = matches!(first_word, "else" | "elif" | "empty");
        let is_block =
            crate::tags::is_django_block_tag(first_word, &config.custom_blocks) || is_middle;

        pending.push_str(&rest[..pos]);
        if !is_block {
            // Not a structural tag (e.g. `{% url %}`, `{% trans %}`): keep it
            // inline with the surrounding text.
            pending.push_str(tag);
            rest = &rest[tag_end..];
            continue;
        }

        any_block = true;
        if !head_done {
            head = pending.trim_end().to_string();
            pending.clear();
            head_done = true;
        } else {
            flush_pending(&mut pending, &mut lines, level);
        }

        if is_closer {
            level = level.saturating_sub(1);
            lines.push(AttrLine {
                level,
                text: tag.to_string(),
            });
        } else if is_middle {
            lines.push(AttrLine {
                level: level.saturating_sub(1),
                text: tag.to_string(),
            });
        } else {
            lines.push(AttrLine {
                level,
                text: tag.to_string(),
            });
            level += 1;
        }
        rest = &rest[tag_end..];
    }

    if !any_block {
        return None;
    }

    pending.push_str(rest);
    if !head_done {
        head = pending.trim_end().to_string();
    } else {
        // djlint breaks after the last block tag, so whatever follows it goes
        // on its own line — including the closing quote when the trailing text
        // is empty (`{% endif %}` then `">`, not `{% endif %}">`).
        lines.push(AttrLine {
            level,
            text: pending.trim().to_string(),
        });
    }

    condense_attr_lines(&mut lines);

    // Render: head stays on the tag line; each remaining line gets a newline
    // and absolute indentation.
    let mut out = head;
    for line in &lines {
        out.push('\n');
        out.push_str(&" ".repeat(line.level * config.indent));
        out.push_str(&line.text);
    }
    Some(out)
}

/// Rejoin `{% if %}` … `{% endif %}` blocks that contain a single text child
/// and no intermediate template tags onto one line, matching djlint's
/// condense step (which leaves `{% else %}`/nested blocks expanded).
fn condense_attr_lines(lines: &mut Vec<AttrLine>) {
    let mut i = 0;
    while i < lines.len() {
        let is_opener = is_block_opener_line(&lines[i].text);
        if is_opener {
            // opener directly followed by its closer (empty body)
            if i + 1 < lines.len() && is_block_closer_line(&lines[i + 1].text) {
                let merged = format!("{}{}", lines[i].text, lines[i + 1].text);
                let level = lines[i].level;
                lines.splice(
                    i..i + 2,
                    [AttrLine {
                        level,
                        text: merged,
                    }],
                );
                continue;
            }
            // opener, one text child, closer
            if i + 2 < lines.len()
                && !is_template_tag_line(&lines[i + 1].text)
                && is_block_closer_line(&lines[i + 2].text)
            {
                let merged = format!(
                    "{}{}{}",
                    lines[i].text,
                    lines[i + 1].text,
                    lines[i + 2].text
                );
                let level = lines[i].level;
                lines.splice(
                    i..i + 3,
                    [AttrLine {
                        level,
                        text: merged,
                    }],
                );
                continue;
            }
        }
        i += 1;
    }
}

/// Whether `text` is exactly one `{% … %}` tag (no interior `%}`), so an
/// already-condensed line like `{% if %},{% endif %}` is not mistaken for a
/// bare opener and merged with a following closer.
fn is_template_tag_line(text: &str) -> bool {
    text.starts_with("{%") && text.ends_with("%}") && !text[2..text.len() - 2].contains("%}")
}

fn is_block_opener_line(text: &str) -> bool {
    is_template_tag_line(text) && {
        let word = text[2..text.len() - 2]
            .split_whitespace()
            .next()
            .unwrap_or("");
        !word.starts_with("end") && !matches!(word, "else" | "elif" | "empty")
    }
}

fn is_block_closer_line(text: &str) -> bool {
    is_template_tag_line(text)
        && text[2..text.len() - 2]
            .split_whitespace()
            .next()
            .unwrap_or("")
            .starts_with("end")
}

/// Whether djlint's "is this template tag inside an html tag" back-scan would
/// be blocked before the value's first block template tag — i.e. the prefix up
/// to that tag contains a `>` (e.g. from an HTML comment or arrow) or a bare
/// `{`/`}`. When blocked, djlint treats the block tags as loose and expands
/// them onto their own lines.
fn backscan_blocked(value: &str, custom_blocks: &[String]) -> bool {
    let mut offset = 0;
    while let Some(rel) = value[offset..].find("{%") {
        let start = offset + rel;
        let Some(end_rel) = value[start..].find("%}") else {
            break;
        };
        let end = start + end_rel + 2;
        let word = value[start + 2..end - 2]
            .split_whitespace()
            .next()
            .unwrap_or("");
        if crate::tags::is_django_block_tag(word, custom_blocks)
            || matches!(word, "else" | "elif" | "empty")
        {
            let prefix = &value[..start];
            return prefix.contains('>') || value_has_bare_brace(prefix);
        }
        offset = end;
    }
    false
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
