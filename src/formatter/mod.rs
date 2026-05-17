pub mod tokenizer;

use crate::config::Config;
use tokenizer::{Token, Tokenizer};

pub fn format(config: &Config, source: &str) -> String {
    let mut output = String::new();
    let mut indent_level = 0;
    let tokens: Vec<Token> = Tokenizer::new(source).collect();
    let mut i = 0;

    while i < tokens.len() {
        match &tokens[i] {
            Token::Doctype { .. } => {
                output.push_str("<!DOCTYPE html>\n");
            }
            Token::Comment { raw, .. } => {
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

                    if raw.len() > config.max_attribute_length && !is_closing {
                        // Reformat tag with wrapped attributes
                        let formatted_tag = format_tag_with_wrapping(
                            name,
                            raw,
                            *is_self_closing,
                            indent_level,
                            config,
                        );
                        output.push_str(&formatted_tag);
                    } else {
                        output.push_str(raw);
                    }

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
                                    Token::Text { .. } => true,
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

    output
}

fn format_tag_with_wrapping(
    name: &str,
    raw: &str,
    is_self_closing: bool,
    indent_level: usize,
    config: &Config,
) -> String {
    // Basic attribute extractor (very simple for now)
    // We expect raw to be like <tag attr="val" attr2="val2">
    let content = raw
        .trim_start_matches('<')
        .trim_end_matches('>')
        .trim_end_matches('/');
    let mut parts = content.split_whitespace();
    let _tag_name = parts.next(); // Skip tag name

    let attrs: Vec<&str> = parts.collect();
    if attrs.is_empty() {
        return raw.to_string();
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

    if is_self_closing {
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
