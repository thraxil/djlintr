pub mod tokenizer;

use crate::config::Config;
use regex::Regex;
use tokenizer::{Token, Tokenizer};

fn is_inline_tag(name: &str) -> bool {
    let inline_tags = [
        "a", "abbr", "acronym", "b", "bdo", "big", "cite", "code", "dfn", "em", "i", "img", "kbd",
        "map", "object", "q", "samp", "small", "span", "strong", "sub", "sup", "tt", "var",
        "title",
    ];
    inline_tags.contains(&name.to_lowercase().as_str())
}

fn is_html_block_tag(name: &str) -> bool {
    let block_tags = [
        "address",
        "article",
        "aside",
        "blockquote",
        "body",
        "canvas",
        "details",
        "dd",
        "div",
        "dl",
        "dt",
        "fieldset",
        "figcaption",
        "figure",
        "footer",
        "form",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "head",
        "header",
        "hr",
        "html",
        "li",
        "main",
        "nav",
        "noscript",
        "ol",
        "p",
        "pre",
        "section",
        "table",
        "tbody",
        "td",
        "tfoot",
        "th",
        "thead",
        "tr",
        "ul",
        "video",
        "svg",
    ];
    block_tags.contains(&name.to_lowercase().as_str())
}

fn is_structural_tag(name: &str) -> bool {
    let structural_tags = [
        "table", "tbody", "thead", "tfoot", "ul", "ol", "dl", "form", "dd",
    ];
    structural_tags.contains(&name.to_lowercase().as_str())
}

fn should_indent_children(name: &str) -> bool {
    let no_indent_tags = [
        "g",
        "defs",
        "clippath",
        "mask",
        "pattern",
        "lineargradient",
        "radialgradient",
        "symbol",
        "marker",
    ];
    !no_indent_tags.contains(&name.to_lowercase().as_str())
}

fn should_wrap_attributes(name: &str) -> bool {
    let no_wrap_tags = [
        "path", "circle", "rect", "line", "polyline", "polygon", "ellipse",
    ];
    !no_wrap_tags.contains(&name.to_lowercase().as_str())
}

pub fn format(config: &Config, source: &str) -> String {
    let mut output = String::new();
    let mut indent_level: usize = 0;
    let tokens: Vec<Token> = Tokenizer::new(source).collect();
    let mut i = 0;
    let mut formatting_enabled = true;
    let mut verbatim_tags = Vec::new();
    let mut at_start_of_line = true;
    let mut parent_stack: Vec<usize> = Vec::new();

    while i < tokens.len() {
        let token = &tokens[i];
        let raw = token.raw();

        // Check for djlint:off/on
        match token {
            Token::Comment { .. } | Token::DjangoBlock { .. } | Token::DjangoComment { .. }
                if raw.contains("djlint:off") =>
            {
                formatting_enabled = false;
            }
            _ => {}
        }

        if !formatting_enabled {
            if let Token::Comment { .. } | Token::DjangoBlock { .. } | Token::DjangoComment { .. } =
                token
            {
                if raw.contains("djlint:on") {
                    formatting_enabled = true;
                    trim_trailing_whitespace(&mut output);
                    output.push_str(raw.trim());
                    output.push('\n');
                    at_start_of_line = true;
                    i += 1;
                    continue;
                }
            }
            output.push_str(raw);
            at_start_of_line = raw.ends_with('\n');
            i += 1;
            continue;
        }

        match token {
            Token::Doctype { .. } => {
                output.push_str(&" ".repeat(indent_level * config.indent));
                output.push_str("<!DOCTYPE html>\n");
                at_start_of_line = true;
            }
            Token::Comment { raw, .. } | Token::DjangoComment { raw, .. } => {
                if !raw.contains("djlint:off") && !raw.contains("djlint:on") {
                    output.push_str(&" ".repeat(indent_level * config.indent));
                }
                output.push_str(raw.trim());
                output.push('\n');
                at_start_of_line = true;
            }
            Token::Tag {
                name,
                raw,
                is_closing,
                is_self_closing,
                ..
            } => {
                let name_lower = name.to_lowercase();
                if *is_closing {
                    if verbatim_tags.last() == Some(&name_lower) {
                        verbatim_tags.pop();
                    }
                    if should_indent_children(name) {
                        indent_level = indent_level.saturating_sub(1);
                    }
                    if !at_start_of_line && !is_inline_tag(name) {
                        trim_trailing_whitespace(&mut output);
                        output.push('\n');
                        at_start_of_line = true;
                    }
                    if at_start_of_line {
                        output.push_str(&" ".repeat(indent_level * config.indent));
                    }
                    output.push_str(&format!("</{}>", name));

                    let mut should_newline = true;
                    if is_inline_tag(name) && i + 1 < tokens.len() {
                        let next_token = &tokens[i + 1];
                        if next_token.line() == token.ends_on_line()
                            && is_inline_ish(next_token, config)
                        {
                            should_newline = false;
                        }
                        if let Token::Text { raw: r, .. } = next_token {
                            if r.starts_with('\n') || r.starts_with("\r\n") {
                                should_newline = false;
                            }
                        }
                    }

                    if should_newline {
                        output.push('\n');
                        at_start_of_line = true;
                    } else {
                        at_start_of_line = false;
                    }
                } else {
                    let (children, closing_idx) = get_children_info(i, &tokens);

                    let is_verbatim = matches!(name_lower.as_str(), "style" | "script")
                        && !is_self_closing
                        && children.iter().any(|&idx| {
                            let child = &tokens[idx];
                            if let Token::Text { raw, .. } = child {
                                raw.trim().contains('\n')
                            } else {
                                child.raw().contains('\n')
                            }
                        });

                    if is_verbatim {
                        verbatim_tags.push(name_lower.clone());
                    }

                    if at_start_of_line {
                        output.push_str(&" ".repeat(indent_level * config.indent));
                    }

                    let formatted_tag =
                        format_tag(name, raw, *is_self_closing, indent_level, config);
                    output.push_str(&formatted_tag);

                    // Check if we can inline
                    if !is_self_closing && !is_verbatim {
                        if let Some(j) = closing_idx {
                            let logical_elements = get_logical_elements(&children, &tokens);

                            let all_inline_ish = logical_elements.iter().all(|range| {
                                if range.len() == 1 {
                                    is_strictly_inline(&tokens[range.start], config)
                                } else {
                                    // It's a tag pair. Inline if it's an inline tag and its content is inlinable.
                                    if let Token::Tag { name: n, .. } = &tokens[range.start] {
                                        is_inline_tag(n)
                                            && is_tag_range_inlinable(range, &tokens, config)
                                    } else {
                                        false
                                    }
                                }
                            });

                            let is_block_parent = is_html_block_tag(name);
                            let is_structural = is_structural_tag(name);

                            let has_any_tag = logical_elements.iter().any(|range| {
                                if range.len() > 1 {
                                    true
                                } else {
                                    let token = &tokens[range.start];
                                    matches!(
                                        token,
                                        Token::Tag { .. }
                                            | Token::Comment { .. }
                                            | Token::DjangoComment { .. }
                                    )
                                }
                            });

                            if all_inline_ish {
                                let can_collapse = if is_structural {
                                    false
                                } else if is_block_parent {
                                    !has_any_tag
                                } else {
                                    true
                                };

                                if can_collapse {
                                    // Collapse
                                    let content = format_range_inlined_joined(
                                        &logical_elements,
                                        &tokens,
                                        indent_level,
                                        config,
                                    );
                                    let collapsed_content = content.trim();
                                    let tag_last_line_len =
                                        formatted_tag.split('\n').next_back().unwrap_or("").len();
                                    let current_line_len = if formatted_tag.contains('\n') {
                                        tag_last_line_len
                                    } else {
                                        (indent_level * config.indent) + tag_last_line_len
                                    };
                                    let projected_len =
                                        current_line_len + collapsed_content.len() + name.len() + 3;

                                    if projected_len <= config.max_line_length
                                        || logical_elements.is_empty()
                                    {
                                        output.push_str(collapsed_content);
                                        output.push_str(&format!("</{}>", name));
                                        let mut should_newline = true;
                                        if is_inline_tag(name) && i + 1 < tokens.len() {
                                            let next_token = &tokens[i + 1];
                                            if next_token.line() == tokens[j].ends_on_line()
                                                && is_inline_ish(next_token, config)
                                            {
                                                should_newline = false;
                                            }
                                        }
                                        if should_newline {
                                            trim_trailing_whitespace(&mut output);
                                            output.push('\n');
                                            at_start_of_line = true;
                                        } else {
                                            at_start_of_line = false;
                                        }
                                        i = j + 1;
                                        continue;
                                    }
                                }

                                // If not collapsed, we still might join children on one line if they are all inline-ish
                                // and the parent is NOT structural.
                                if !is_structural
                                    && !logical_elements.is_empty()
                                    && !is_block_parent
                                {
                                    let content = format_range_inlined_joined(
                                        &logical_elements,
                                        &tokens,
                                        indent_level + 1,
                                        config,
                                    );
                                    let collapsed_content = content.trim();

                                    trim_trailing_whitespace(&mut output);
                                    output.push('\n');
                                    output
                                        .push_str(&" ".repeat((indent_level + 1) * config.indent));
                                    output.push_str(collapsed_content);
                                    trim_trailing_whitespace(&mut output);
                                    output.push('\n');
                                    output.push_str(&" ".repeat(indent_level * config.indent));
                                    output.push_str(&format!("</{}>", name));
                                    let mut should_newline = true;
                                    if is_inline_tag(name) && i + 1 < tokens.len() {
                                        let next_token = &tokens[i + 1];
                                        if next_token.line() == tokens[j].ends_on_line()
                                            && is_inline_ish(next_token, config)
                                        {
                                            should_newline = false;
                                        }
                                    }
                                    if should_newline {
                                        trim_trailing_whitespace(&mut output);
                                        output.push('\n');
                                        at_start_of_line = true;
                                    } else {
                                        at_start_of_line = false;
                                    }
                                    i = j + 1;
                                    continue;
                                }
                            }
                        }
                    }

                    if !is_verbatim {
                        let mut should_newline = true;
                        if is_inline_tag(name) && i + 1 < tokens.len() {
                            let next_token = &tokens[i + 1];
                            if next_token.line() == token.ends_on_line()
                                && is_inline_ish(next_token, config)
                            {
                                should_newline = false;
                            }
                            if let Token::Text { raw: r, .. } = next_token {
                                if r.starts_with('\n') || r.starts_with("\r\n") {
                                    should_newline = false;
                                }
                            }
                        }
                        if should_newline {
                            trim_trailing_whitespace(&mut output);
                            output.push('\n');
                            at_start_of_line = true;
                        } else {
                            at_start_of_line = false;
                        }
                    } else {
                        at_start_of_line = false;
                    }

                    if !is_self_closing && should_indent_children(name) {
                        indent_level += 1;
                    }
                    if !is_self_closing {
                        parent_stack.push(i);
                    }
                }
            }
            Token::Text { raw, .. } => {
                if !verbatim_tags.is_empty() {
                    output.push_str(raw);
                    at_start_of_line = raw.ends_with('\n');
                } else {
                    let trimmed = raw.trim();
                    if !trimmed.is_empty() {
                        let lines: Vec<&str> = trimmed.lines().collect();
                        for (idx, line) in lines.iter().enumerate() {
                            let is_last_line = idx == lines.len() - 1;

                            if at_start_of_line {
                                output.push_str(&" ".repeat(indent_level * config.indent));
                                output.push_str(line.trim_start());
                            } else if idx == 0 {
                                // Continuing inline. We want to preserve original leading spaces
                                // but only if they weren't trimmed away by lines()
                                if raw.starts_with(" ") && !line.starts_with(" ") {
                                    output.push(' ');
                                }
                                output.push_str(line);
                            } else {
                                output.push_str(line.trim_start());
                            }

                            if is_last_line {
                                let mut should_newline = true;
                                if i + 1 < tokens.len() {
                                    let next_token = &tokens[i + 1];
                                    if next_token.line() == token.ends_on_line()
                                        && is_inline_ish(next_token, config)
                                    {
                                        should_newline = false;
                                    }
                                    if let Token::Text { raw: r, .. } = next_token {
                                        if r.starts_with('\n') || r.starts_with("\r\n") {
                                            should_newline = false;
                                        }
                                    }
                                }

                                if should_newline {
                                    trim_trailing_whitespace(&mut output);
                                    output.push('\n');
                                    at_start_of_line = true;
                                } else {
                                    // Preserve original trailing space if any
                                    if raw.ends_with(" ") && !output.ends_with(" ") {
                                        output.push(' ');
                                    }
                                    at_start_of_line = false;
                                }
                            } else {
                                output.push('\n');
                                at_start_of_line = true;
                            }
                        }
                    } else if raw.contains('\n') {
                        if !at_start_of_line {
                            trim_trailing_whitespace(&mut output);
                            output.push('\n');
                        }
                        at_start_of_line = true;
                    }
                }
            }
            Token::DjangoVar { raw, .. } => {
                if at_start_of_line {
                    output.push_str(&" ".repeat(indent_level * config.indent));
                }
                output.push_str(&normalize_django(raw));

                let mut should_newline = true;
                if i + 1 < tokens.len() {
                    let next_token = &tokens[i + 1];
                    if next_token.line() == token.ends_on_line()
                        && is_inline_ish(next_token, config)
                    {
                        should_newline = false;
                    }
                    if let Token::Text { raw: r, .. } = next_token {
                        if r.starts_with('\n') || r.starts_with("\r\n") {
                            should_newline = false;
                        }
                    }
                }

                if should_newline {
                    trim_trailing_whitespace(&mut output);
                    output.push('\n');
                    at_start_of_line = true;
                } else {
                    at_start_of_line = false;
                }
            }
            Token::DjangoBlock { raw, .. } => {
                let tag_name = get_django_tag_name(raw).unwrap_or("");
                let is_closing = tag_name.starts_with("end");
                let actual_tag_name = if is_closing { &tag_name[3..] } else { tag_name };
                let is_block = is_block_tag(actual_tag_name, &config.custom_blocks);
                let is_reindent = is_reindent_tag(tag_name);

                if (is_closing && is_block) || is_reindent {
                    indent_level = indent_level.saturating_sub(1);
                }

                // Check if we can inline
                if !is_closing && is_block {
                    let (children, closing_idx) = get_children_info(i, &tokens);

                    if let Some(j) = closing_idx {
                        let logical_elements = get_logical_elements(&children, &tokens);

                        let has_any_tag_or_django = logical_elements.iter().any(|range| {
                            range.len() > 1 || !matches!(tokens[range.start], Token::Text { .. })
                        });

                        let all_inline_ish = logical_elements.iter().all(|range| {
                            if range.len() == 1 {
                                is_strictly_inline(&tokens[range.start], config)
                            } else {
                                // It's a tag pair.
                                if let Token::Tag { name: n, .. } = &tokens[range.start] {
                                    let is_block = is_html_block_tag(n);
                                    if is_block {
                                        // Don't inline block tags if they contain other tags
                                        let children_indices: Vec<usize> =
                                            (range.start + 1..range.end - 1).collect();
                                        let sub_elements =
                                            get_logical_elements(&children_indices, &tokens);
                                        let has_sub_tag = sub_elements.iter().any(|r| r.len() > 1);
                                        !has_sub_tag
                                            && is_tag_range_inlinable(range, &tokens, config)
                                    } else {
                                        is_tag_range_inlinable(range, &tokens, config)
                                    }
                                } else {
                                    false
                                }
                            }
                        });

                        if all_inline_ish {
                            let normalized_start = normalize_django(raw);
                            let normalized_end = normalize_django(tokens[j].raw());
                            let content = format_range_inlined_joined(
                                &logical_elements,
                                &tokens,
                                indent_level + 1,
                                config,
                            );
                            let collapsed_content = content.trim();
                            let projected_len = (indent_level * config.indent)
                                + normalized_start.len()
                                + collapsed_content.len()
                                + normalized_end.len();

                            if projected_len <= config.max_line_length {
                                if at_start_of_line {
                                    output.push_str(&" ".repeat(indent_level * config.indent));
                                }
                                output.push_str(&normalized_start);
                                output.push_str(collapsed_content);
                                output.push_str(&normalized_end);
                                trim_trailing_whitespace(&mut output);
                                output.push('\n');
                                at_start_of_line = true;
                                i = j + 1;
                                continue;
                            } else if !logical_elements.is_empty() && !has_any_tag_or_django {
                                // Expand but join children
                                if at_start_of_line {
                                    output.push_str(&" ".repeat(indent_level * config.indent));
                                }
                                output.push_str(&normalized_start);
                                trim_trailing_whitespace(&mut output);
                                output.push('\n');
                                output.push_str(&" ".repeat((indent_level + 1) * config.indent));
                                output.push_str(collapsed_content);
                                trim_trailing_whitespace(&mut output);
                                output.push('\n');
                                output.push_str(&" ".repeat(indent_level * config.indent));
                                output.push_str(&normalized_end);
                                trim_trailing_whitespace(&mut output);
                                output.push('\n');
                                at_start_of_line = true;
                                i = j + 1;
                                continue;
                            }
                        }
                    }
                }

                if at_start_of_line {
                    output.push_str(&" ".repeat(indent_level * config.indent));
                }
                output.push_str(&normalize_django(raw));

                let mut should_newline = true;
                if !is_block && !is_reindent && i + 1 < tokens.len() {
                    let next_token = &tokens[i + 1];
                    if next_token.line() == token.ends_on_line()
                        && is_inline_ish(next_token, config)
                    {
                        should_newline = false;
                    }
                    if let Token::Text { raw: r, .. } = next_token {
                        if r.starts_with('\n') || r.starts_with("\r\n") {
                            should_newline = false;
                        }
                    }
                }

                if should_newline {
                    trim_trailing_whitespace(&mut output);
                    output.push('\n');
                    at_start_of_line = true;
                } else {
                    at_start_of_line = false;
                }

                if (!is_closing && is_block) || is_reindent {
                    let (_, closing_idx) = get_children_info(i, &tokens);
                    if closing_idx.is_some() || is_reindent {
                        indent_level += 1;
                        parent_stack.push(i);
                    }
                }
            }
        }

        // Pop from parent_stack if we just finished a closing tag or a closing django block
        let token = &tokens[i];
        match token {
            Token::Tag {
                is_closing: true, ..
            } => {
                parent_stack.pop();
            }
            Token::DjangoBlock { raw, .. } => {
                let tag_name = get_django_tag_name(raw).unwrap_or("");
                if tag_name.starts_with("end") {
                    parent_stack.pop();
                }
            }
            _ => {}
        }

        i += 1;
    }

    if !output.is_empty() && !output.ends_with('\n') {
        output.push('\n');
    }

    output
}

fn normalize_django(raw: &str) -> String {
    let var_re = Regex::new(r#"\{\{[\s\S]*?\}\}"#).unwrap();
    let block_re = Regex::new(r#"\{%[\s\S]*?%\}"#).unwrap();

    let mut result = raw.to_string();

    // Replace vars
    result = var_re
        .replace_all(&result, |caps: &regex::Captures| {
            let m = caps.get(0).unwrap().as_str();
            let content = m[2..m.len() - 2].trim();
            format!("{{{{ {} }}}}", content)
        })
        .to_string();

    // Replace blocks
    result = block_re
        .replace_all(&result, |caps: &regex::Captures| {
            let m = caps.get(0).unwrap().as_str();
            let content = m[2..m.len() - 2].trim();
            format!("{{% {} %}}", content)
        })
        .to_string();

    result
}

fn trim_trailing_whitespace(s: &mut String) {
    let current_trimmed = s.trim_end_matches([' ', '\t']);
    s.truncate(current_trimmed.len());
}

fn format_tag(
    name: &str,
    raw: &str,
    is_self_closing: bool,
    indent_level: usize,
    config: &Config,
) -> String {
    let attr_re = Regex::new(
        r#"([a-zA-Z0-9:@._#*!-]+(?:\s*=\s*(?:"(?:\{\{[\s\S]*?\}\}|\{%[\s\S]*?%\}|[^"])*"|'(?:\{\{[\s\S]*?\}\}|\{%[\s\S]*?%\}|[^'])*'|[^\s>]+))?|\{\{[\s\S]*?\}\}|\{%[\s\S]*?%\}(?:\s*\{%[\s\S]*?%\})*)"#,
    )
    .unwrap();

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

    let attrs: Vec<String> = attr_re
        .find_iter(content)
        .map(|m| normalize_django(m.as_str()))
        .collect();

    let final_content = attr_re
        .replace_all(content, |caps: &regex::Captures| {
            let m = caps.get(0).unwrap().as_str();
            if m.starts_with("style=") {
                format_style_attribute(m, "")
            } else {
                normalize_django(m)
            }
        })
        .to_string();

    let attrs_total_len = if attrs.is_empty() {
        0
    } else {
        attrs.iter().map(|a| a.len()).sum::<usize>() + attrs.len() - 1
    };

    if (attrs_total_len <= config.max_attribute_length || !should_wrap_attributes(name))
        && !raw.contains('\n')
    {
        let mut formatted = if raw.starts_with("</") {
            format!("</{}", name)
        } else {
            format!("<{}", name)
        };

        formatted.push_str(final_content.trim_end());

        if raw.ends_with("/>") || (is_self_closing && config.close_void_tags) {
            formatted.push_str(" />");
        } else {
            formatted.push('>');
        }
        return formatted;
    }

    let mut formatted = format!("<{}", name);
    let attr_indent = " ".repeat(indent_level * config.indent + name.len() + 2);

    let mut attrs_iter = attrs.into_iter();
    if let Some(attr) = attrs_iter.next() {
        formatted.push(' ');
        if attr.starts_with("style=") {
            formatted.push_str(&format_style_attribute(&attr, &attr_indent));
        } else {
            formatted.push_str(&attr);
        }
    }

    for attr in attrs_iter {
        formatted.push('\n');
        formatted.push_str(&attr_indent);
        if attr.starts_with("style=") {
            formatted.push_str(&format_style_attribute(&attr, &attr_indent));
        } else {
            formatted.push_str(&attr);
        }
    }

    if raw.ends_with("/>") || (is_self_closing && config.close_void_tags) {
        formatted.push_str(" />");
    } else {
        formatted.push('>');
    }

    formatted
}

fn format_style_attribute(attr: &str, indent: &str) -> String {
    // Expect style="prop1: val1; prop2: val2;"
    let quote = if attr.contains("=\"") { "\"" } else { "'" };
    let content_start = attr.find(quote).unwrap_or(0) + 1;
    let content_end = attr.rfind(quote).unwrap_or(attr.len());
    let content = &attr[content_start..content_end];

    let props: Vec<&str> = content
        .split(';')
        .map(|s| s.trim())
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

fn get_django_tag_name(raw: &str) -> Option<&str> {
    raw.trim_start_matches("{%").split_whitespace().next()
}

fn is_reindent_tag(name: &str) -> bool {
    matches!(name, "else" | "elif" | "empty")
}

fn is_strictly_inline(token: &Token, config: &Config) -> bool {
    match token {
        Token::DjangoVar { .. } => true,
        Token::DjangoBlock { raw, .. } => {
            let tag_name = get_django_tag_name(raw).unwrap_or("");
            if is_reindent_tag(tag_name) {
                return false;
            }
            let is_closing = tag_name.starts_with("end");
            let actual_tag_name = if is_closing { &tag_name[3..] } else { tag_name };
            !is_block_tag(actual_tag_name, &config.custom_blocks)
        }
        Token::Text { raw, .. } => !raw.trim().contains('\n'),
        Token::Tag { name, .. } => is_inline_tag(name),
        Token::Comment { raw, .. } | Token::DjangoComment { raw, .. } => !raw.contains('\n'),
        Token::Doctype { .. } => false,
    }
}

fn is_inline_ish(token: &Token, config: &Config) -> bool {
    match token {
        Token::DjangoVar { .. } => true,
        Token::DjangoBlock { raw, .. } => {
            let tag_name = get_django_tag_name(raw).unwrap_or("");
            if is_reindent_tag(tag_name) {
                return false;
            }
            let is_closing = tag_name.starts_with("end");
            let actual_tag_name = if is_closing { &tag_name[3..] } else { tag_name };
            !is_block_tag(actual_tag_name, &config.custom_blocks)
        }
        Token::Text { raw, .. } => raw
            .split('\n')
            .next()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false),
        Token::Tag { name, .. } => is_inline_tag(name),
        Token::Comment { raw, .. } | Token::DjangoComment { raw, .. } => !raw.contains('\n'),
        Token::Doctype { .. } => false,
    }
}

fn is_block_tag(name: &str, custom_blocks: &[String]) -> bool {
    let blocks = [
        "block",
        "if",
        "for",
        "with",
        "autoescape",
        "filter",
        "spaceless",
        "cache",
        "macro",
        "call",
        "set",
        "localize",
        "compress",
        "comment",
        "load",
        "extends",
    ];
    blocks.contains(&name) || custom_blocks.iter().any(|b| b == name)
}

fn get_children_info(index: usize, tokens: &[Token]) -> (Vec<usize>, Option<usize>) {
    let mut children = Vec::new();
    let token = &tokens[index];
    match token {
        Token::Tag { name, .. } => {
            let mut j = index + 1;
            let mut depth = 1;
            while j < tokens.len() {
                match &tokens[j] {
                    Token::Tag {
                        name: n,
                        is_closing: false,
                        is_self_closing: false,
                        ..
                    } if n == name => {
                        depth += 1;
                    }
                    Token::Tag {
                        name: n,
                        is_closing: true,
                        ..
                    } if n == name => {
                        depth -= 1;
                        if depth == 0 {
                            return (children, Some(j));
                        }
                    }
                    _ => {}
                }
                children.push(j);
                j += 1;
            }
        }
        Token::DjangoBlock { raw, .. } => {
            let tag_name = get_django_tag_name(raw).unwrap_or("");
            let end_tag_name = format!("end{}", tag_name);
            let mut j = index + 1;
            let mut depth = 1;
            while j < tokens.len() {
                if let Token::DjangoBlock { raw: r, .. } = &tokens[j] {
                    let name = get_django_tag_name(r).unwrap_or("");
                    if name == tag_name {
                        depth += 1;
                    } else if name == end_tag_name {
                        depth -= 1;
                        if depth == 0 {
                            return (children, Some(j));
                        }
                    }
                }
                children.push(j);
                j += 1;
            }
        }
        _ => {}
    }
    (children, None)
}

fn get_logical_elements(children: &[usize], tokens: &[Token]) -> Vec<std::ops::Range<usize>> {
    let mut elements = Vec::new();
    let mut i = 0;
    while i < children.len() {
        let idx = children[i];
        match &tokens[idx] {
            Token::Text { raw, .. } if raw.trim().is_empty() && raw.contains('\n') => {
                i += 1;
            }
            Token::Tag {
                name,
                is_closing: false,
                is_self_closing: false,
                ..
            } => {
                let start = idx;
                let mut depth = 1;
                i += 1;
                while i < children.len() && depth > 0 {
                    let next_idx = children[i];
                    match &tokens[next_idx] {
                        Token::Tag {
                            name: n,
                            is_closing: false,
                            is_self_closing: false,
                            ..
                        } if n == name => {
                            depth += 1;
                        }
                        Token::Tag {
                            name: n,
                            is_closing: true,
                            ..
                        } if n == name => {
                            depth -= 1;
                        }
                        _ => {}
                    }
                    i += 1;
                }
                elements.push(start..children[i - 1] + 1);
            }
            _ => {
                elements.push(idx..idx + 1);
                i += 1;
            }
        }
    }
    elements
}

fn is_tag_range_inlinable(
    range: &std::ops::Range<usize>,
    tokens: &[Token],
    config: &Config,
) -> bool {
    let children_indices: Vec<usize> = (range.start + 1..range.end - 1).collect();
    let logical_elements = get_logical_elements(&children_indices, tokens);

    if logical_elements.is_empty() {
        return true;
    }

    logical_elements.iter().all(|range| {
        if range.len() == 1 {
            let child = &tokens[range.start];
            if let Token::Text { raw, .. } = child {
                !raw.trim().contains('\n')
            } else if let Token::DjangoVar { .. } = child {
                true
            } else if let Token::DjangoBlock { raw, .. } = child {
                let tag_name = get_django_tag_name(raw).unwrap_or("");
                !is_block_tag(tag_name, &config.custom_blocks)
            } else {
                false
            }
        } else {
            // It's a tag pair.
            if let Token::Tag { name: n, .. } = &tokens[range.start] {
                is_inline_tag(n) && is_tag_range_inlinable(range, tokens, config)
            } else {
                false
            }
        }
    })
}

fn format_range_inlined(
    range: &std::ops::Range<usize>,
    tokens: &[Token],
    indent_level: usize,
    config: &Config,
) -> String {
    let mut result = String::new();
    let mut k = range.start;
    while k < range.end {
        let token = &tokens[k];
        match token {
            Token::Tag {
                name,
                raw,
                is_closing,
                is_self_closing,
                ..
            } => {
                if *is_closing {
                    result.push_str(&format!("</{}>", name));
                } else {
                    let (children, closing_idx) = get_children_info(k, tokens);
                    if let Some(j) = closing_idx {
                        if range.contains(&j) {
                            result.push_str(&format_tag(
                                name,
                                raw,
                                *is_self_closing,
                                indent_level,
                                config,
                            ));
                            let sub_elements = get_logical_elements(&children, tokens);
                            let inner_content = format_range_inlined_joined(
                                &sub_elements,
                                tokens,
                                indent_level,
                                config,
                            );
                            result.push_str(&inner_content);
                            result.push_str(&format!("</{}>", name));
                            k = j;
                        } else {
                            result.push_str(&format_tag(
                                name,
                                raw,
                                *is_self_closing,
                                indent_level,
                                config,
                            ));
                        }
                    } else {
                        result.push_str(&format_tag(
                            name,
                            raw,
                            *is_self_closing,
                            indent_level,
                            config,
                        ));
                    }
                }
            }
            Token::DjangoVar { raw, .. } | Token::DjangoBlock { raw, .. } => {
                result.push_str(&normalize_django(raw));
            }
            Token::Text { raw, .. } => {
                if raw.contains('\n') {
                    result.push_str(raw.trim_matches(|c| c == '\n' || c == '\r'));
                } else {
                    result.push_str(raw);
                }
            }
            _ => {
                result.push_str(token.raw());
            }
        }
        k += 1;
    }
    result
}

fn format_range_inlined_joined(
    logical_elements: &[std::ops::Range<usize>],
    tokens: &[Token],
    indent_level: usize,
    config: &Config,
) -> String {
    let mut content = String::new();
    for (k, range) in logical_elements.iter().enumerate() {
        let mut element_content = format_range_inlined(range, tokens, indent_level, config);
        if k == 0 {
            element_content = element_content.trim_start_matches(['\n', '\r']).to_string();
        }
        if k == logical_elements.len() - 1 {
            element_content = element_content.trim_end_matches(['\n', '\r']).to_string();
        }
        content.push_str(&element_content);
    }
    content
}
