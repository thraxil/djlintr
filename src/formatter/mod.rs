pub mod tokenizer;

use crate::config::Config;
use regex::Regex;
use tokenizer::{Token, Tokenizer};

pub fn format(config: &Config, source: &str) -> String {
    let mut output = String::new();
    let mut indent_level = 0;
    let tokens: Vec<Token> = Tokenizer::new(source).collect();
    let mut i = 0;
    let mut formatting_enabled = true;

    while i < tokens.len() {
        let token = &tokens[i];
        let raw = token.raw();

        // Check for djlint:off/on
        match token {
            Token::Comment { .. } | Token::DjangoBlock { .. } | Token::DjangoComment { .. } => {
                if raw.contains("djlint:off") {
                    formatting_enabled = false;
                }
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
                output.push_str("<!DOCTYPE html>\n");
            }
            Token::Comment { raw, .. } | Token::DjangoComment { raw, .. } => {
                output.push_str(&" ".repeat(indent_level * config.indent));
                output.push_str(raw.trim());
                output.push('\n');
            }
            Token::Tag {
                name,
                raw,
                is_closing,
                is_self_closing,
                ..
            } => {
                if *is_closing {
                    indent_level = indent_level.saturating_sub(1);
                    output.push_str(&" ".repeat(indent_level * config.indent));
                    output.push_str(raw);
                    output.push('\n');
                } else {
                    output.push_str(&" ".repeat(indent_level * config.indent));

                    let formatted_tag =
                        format_tag(name, raw, *is_self_closing, indent_level, config);
                    output.push_str(&formatted_tag);

                    // Check if we can inline
                    if !is_self_closing {
                        let mut j = i + 1;
                        let mut children = Vec::new();
                        while j < tokens.len() {
                            match &tokens[j] {
                                Token::Text { raw, .. } if raw.trim().is_empty() => {
                                    j += 1;
                                }
                                Token::Tag {
                                    name: next_name,
                                    is_closing: true,
                                    ..
                                } if next_name == name => {
                                    break;
                                }
                                _ => {
                                    children.push(j);
                                    j += 1;
                                    if children.len() > 1 {
                                        break;
                                    }
                                }
                            }
                        }

                        if j < tokens.len() && children.len() <= 1 {
                            let can_inline = if children.is_empty() {
                                true
                            } else {
                                let child = &tokens[children[0]];
                                match child {
                                    Token::Text { raw, .. } => !raw.trim().contains('\n'),
                                    Token::DjangoVar { .. } => true,
                                    Token::DjangoBlock { raw, .. } => {
                                        let tag_name = get_django_tag_name(raw).unwrap_or("");
                                        !is_block_tag(tag_name, &config.custom_blocks)
                                    }
                                    _ => false,
                                }
                            };

                            if can_inline {
                                if !children.is_empty() {
                                    output.push_str(tokens[children[0]].raw().trim());
                                }
                                output.push_str(tokens[j].raw());
                                output.push('\n');
                                i = j + 1;
                                continue;
                            }
                        }
                    }

                    output.push('\n');
                    if !is_self_closing {
                        indent_level += 1;
                    }
                }
            }
            Token::Text { raw, .. } => {
                let trimmed = raw.trim();
                if !trimmed.is_empty() {
                    output.push_str(&" ".repeat(indent_level * config.indent));
                    output.push_str(trimmed);
                    output.push('\n');
                }
            }
            Token::DjangoVar { raw, .. } => {
                output.push_str(&" ".repeat(indent_level * config.indent));
                output.push_str(raw);
                output.push('\n');
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

                output.push_str(&" ".repeat(indent_level * config.indent));
                output.push_str(raw);
                output.push('\n');

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

    let attrs: Vec<&str> = attr_re.find_iter(content).map(|m| m.as_str()).collect();

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
        for attr in attrs {
            formatted.push(' ');
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

    let mut attrs = attrs.into_iter();
    if let Some(attr) = attrs.next() {
        formatted.push(' ');
        formatted.push_str(attr);
    }

    for attr in attrs {
        formatted.push('\n');
        formatted.push_str(&attr_indent);
        formatted.push_str(attr);
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
