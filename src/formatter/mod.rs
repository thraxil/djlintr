pub mod tokenizer;

use crate::config::Config;
use regex::Regex;
use tokenizer::{Token, Tokenizer};

fn is_inline_tag(name: &str) -> bool {
    let inline_tags = [
        "a", "abbr", "acronym", "b", "bdo", "big", "br", "button", "cite", "code", "dfn", "em",
        "i", "img", "input", "kbd", "label", "map", "object", "q", "samp", "script", "select",
        "small", "span", "strong", "sub", "sup", "textarea", "time", "tt", "var",
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
    ];
    block_tags.contains(&name.to_lowercase().as_str())
}

fn is_structural_tag(name: &str) -> bool {
    let structural_tags = [
        "table", "tbody", "thead", "tfoot", "ul", "ol", "dl", "form", "dd",
    ];
    structural_tags.contains(&name.to_lowercase().as_str())
}

pub fn format(config: &Config, source: &str) -> String {
    let mut output = String::new();
    let mut indent_level: usize = 0;
    let tokens: Vec<Token> = Tokenizer::new(source).collect();
    let mut i = 0;
    let mut formatting_enabled = true;
    let mut verbatim_tags = Vec::new();
    let mut at_start_of_line = true;

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
            output.push_str(raw);
            if let Token::Comment { .. } | Token::DjangoBlock { .. } | Token::DjangoComment { .. } =
                token
            {
                if raw.contains("djlint:on") {
                    formatting_enabled = true;
                }
            }
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
                output.push_str(&" ".repeat(indent_level * config.indent));
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
                    indent_level = indent_level.saturating_sub(1);
                    output.push_str(&" ".repeat(indent_level * config.indent));
                    output.push_str(&format!("</{}>", name));
                    output.push('\n');
                    at_start_of_line = true;
                } else {
                    let (children, closing_idx) = get_children_info(i, &tokens);

                    let is_verbatim = matches!(name_lower.as_str(), "style" | "script")
                        && !is_self_closing
                        && children.iter().any(|&idx| tokens[idx].raw().contains('\n'));

                    if is_verbatim {
                        verbatim_tags.push(name_lower.clone());
                    }

                    output.push_str(&" ".repeat(indent_level * config.indent));

                    let formatted_tag =
                        format_tag(name, raw, *is_self_closing, indent_level, config);
                    output.push_str(&formatted_tag);

                    // Check if we can inline
                    if !is_self_closing && !is_verbatim {
                        if let Some(j) = closing_idx {
                            let logical_elements = get_logical_elements(&children, &tokens);

                            let has_tag = logical_elements.iter().any(|range| range.len() > 1);
                            let all_inline_ish = logical_elements.iter().all(|range| {
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

                            if all_inline_ish {
                                let can_collapse = if is_structural {
                                    false
                                } else if is_block_parent {
                                    // Block parents only collapse if they have no tags and single-line text
                                    !has_tag
                                } else {
                                    // Non-block parents (like span) can collapse mixed inline content
                                    true
                                };

                                if can_collapse {
                                    // Collapse
                                    let mut content = String::new();
                                    for range in logical_elements {
                                        content.push_str(&format_range_inlined(
                                            &range,
                                            &tokens,
                                            indent_level,
                                            config,
                                        ));
                                    }
                                    output.push_str(content.trim());
                                    output.push_str(&format!("</{}>", name));
                                    output.push('\n');
                                    at_start_of_line = true;
                                    i = j + 1;
                                    continue;
                                } else {
                                    // Expand parent, but join children on one line
                                    output.push('\n');
                                    output
                                        .push_str(&" ".repeat((indent_level + 1) * config.indent));
                                    let mut content = String::new();
                                    for range in logical_elements {
                                        content.push_str(&format_range_inlined(
                                            &range,
                                            &tokens,
                                            indent_level + 1,
                                            config,
                                        ));
                                    }
                                    output.push_str(content.trim());
                                    output.push('\n');
                                    output.push_str(&" ".repeat(indent_level * config.indent));
                                    output.push_str(&format!("</{}>", name));
                                    output.push('\n');
                                    at_start_of_line = true;
                                    i = j + 1;
                                    continue;
                                }
                            }
                        }
                    }

                    if !is_verbatim {
                        output.push('\n');
                        at_start_of_line = true;
                    } else {
                        at_start_of_line = false;
                    }

                    if !is_self_closing {
                        indent_level += 1;
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
                        let indent = " ".repeat(indent_level * config.indent);
                        for line in trimmed.lines() {
                            output.push_str(&indent);
                            output.push_str(line.trim());
                            output.push('\n');
                        }
                        at_start_of_line = true;
                    } else if raw.contains('\n') {
                        if !at_start_of_line {
                            output.push('\n');
                        }
                        at_start_of_line = true;
                    } else {
                        output.push_str(raw);
                        at_start_of_line = false;
                    }
                }
            }
            Token::DjangoVar { raw, .. } => {
                output.push_str(&" ".repeat(indent_level * config.indent));
                output.push_str(&normalize_django(raw));
                output.push('\n');
                at_start_of_line = true;
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

                        if logical_elements.len() <= 1 {
                            let can_inline = if logical_elements.is_empty() {
                                true
                            } else {
                                let range = &logical_elements[0];
                                if range.len() == 1 {
                                    let child = &tokens[range.start];
                                    if let Token::Text { raw, .. } = child {
                                        !raw.trim().contains('\n')
                                    } else {
                                        matches!(child, Token::DjangoVar { .. })
                                    }
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
                                            let has_sub_tag =
                                                sub_elements.iter().any(|r| r.len() > 1);
                                            !has_sub_tag
                                                && is_tag_range_inlinable(range, &tokens, config)
                                        } else {
                                            is_tag_range_inlinable(range, &tokens, config)
                                        }
                                    } else {
                                        false
                                    }
                                }
                            };

                            if can_inline {
                                output.push_str(&" ".repeat(indent_level * config.indent));
                                output.push_str(&normalize_django(raw));
                                let mut content = String::new();
                                for range in logical_elements {
                                    content.push_str(&format_range_inlined(
                                        &range,
                                        &tokens,
                                        indent_level + 1,
                                        config,
                                    ));
                                }
                                output.push_str(content.trim());
                                output.push_str(&normalize_django(tokens[j].raw()));
                                output.push('\n');
                                at_start_of_line = true;
                                i = j + 1;
                                continue;
                            }
                        }
                    }
                }

                output.push_str(&" ".repeat(indent_level * config.indent));
                output.push_str(&normalize_django(raw));
                output.push('\n');
                at_start_of_line = true;

                if (!is_closing && is_block) || is_reindent {
                    indent_level += 1;
                }
            }
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

fn format_tag(
    name: &str,
    raw: &str,
    is_self_closing: bool,
    indent_level: usize,
    config: &Config,
) -> String {
    let attr_re = Regex::new(
        r#"([a-zA-Z0-9:@._#*!-]+(?:\s*=\s*(?:"(?:\{\{[\s\S]*?\}\}|\{%[\s\S]*?%\}|[^"])*"|'(?:\{\{[\s\S]*?\}\}|\{%[\s\S]*?%\}|[^'])*'|[^\s>]+))?|\{\{[\s\S]*?\}\}|\{%[\s\S]*?%\})"#,
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

    if attrs.is_empty() {
        if raw.ends_with("/>") || (is_self_closing && config.close_void_tags) {
            return format!("<{} />", name);
        } else {
            return format!("<{}>", name);
        }
    }

    // Check if we should wrap
    let total_len = name.len() + 2 + attrs.iter().map(|a| a.len() + 1).sum::<usize>();

    if total_len <= config.max_attribute_length {
        let mut formatted = format!("<{}", name);
        for (i, attr) in attrs.iter().enumerate() {
            let starts_with_block = attr.starts_with("{%");
            let prev_ends_with_block = if i > 0 {
                attrs[i - 1].ends_with("%}")
            } else {
                false
            };

            if i == 0 || (!starts_with_block && !prev_ends_with_block) {
                formatted.push(' ');
            }
            formatted.push_str(attr);
        }
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
        formatted.push_str(&attr);
    }

    for attr in attrs_iter {
        formatted.push('\n');
        formatted.push_str(&attr_indent);
        formatted.push_str(&attr);
    }

    if raw.ends_with("/>") || (is_self_closing && config.close_void_tags) {
        formatted.push_str(" />");
    } else {
        formatted.push('>');
    }

    formatted
}

fn get_django_tag_name(raw: &str) -> Option<&str> {
    raw.trim_start_matches("{%").split_whitespace().next()
}

fn is_reindent_tag(name: &str) -> bool {
    matches!(name, "else" | "elif" | "empty")
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
    for i in range.clone() {
        let token = &tokens[i];
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
                    result.push_str(&format_tag(
                        name,
                        raw,
                        *is_self_closing,
                        indent_level,
                        config,
                    ));
                }
            }
            Token::DjangoVar { raw, .. } | Token::DjangoBlock { raw, .. } => {
                result.push_str(&normalize_django(raw));
            }
            Token::Text { raw, .. } => {
                if raw.contains('\n') {
                    result.push_str(raw.trim());
                } else {
                    result.push_str(raw);
                }
            }
            _ => {
                result.push_str(token.raw());
            }
        }
    }
    result
}
