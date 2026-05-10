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
                output.push_str("\n");
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
                    output.push_str("\n");
                } else {
                    output.push_str(&" ".repeat(indent_level * config.indent));
                    output.push_str(raw);
                    
                    // Check if next token is the closing tag for this one
                    if !is_self_closing && i + 1 < tokens.len() {
                        if let Token::Tag { 
                            name: next_name, 
                            is_closing: true, 
                            .. 
                        } = &tokens[i+1] {
                            if next_name == name {
                                output.push_str(&tokens[i+1].raw());
                                output.push_str("\n");
                                i += 2;
                                continue;
                            }
                        }
                    }

                    output.push_str("\n");
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
                    output.push_str("\n");
                }
            }
            Token::DjangoVar { raw, .. } => {
                output.push_str(&" ".repeat(indent_level * config.indent));
                output.push_str(raw);
                output.push_str("\n");
            }
            Token::DjangoBlock { raw, .. } => {
                let tag_name = get_django_tag_name(raw).unwrap_or("");
                let is_closing = tag_name.starts_with("end");
                let actual_tag_name = if is_closing {
                    &tag_name[3..]
                } else {
                    tag_name
                };

                let is_block = is_block_tag(actual_tag_name, &config.custom_blocks);
                let is_reindent = is_reindent_tag(tag_name);

                if (is_closing && is_block) || is_reindent {
                    indent_level = indent_level.saturating_sub(1);
                }

                output.push_str(&" ".repeat(indent_level * config.indent));
                output.push_str(raw);
                output.push_str("\n");

                if (!is_closing && is_block) || is_reindent {
                    indent_level += 1;
                }
            }
        }
        i += 1;
    }

    output
}

fn get_django_tag_name(raw: &str) -> Option<&str> {
    raw.trim_start_matches("{%")
        .trim_start()
        .split_whitespace()
        .next()
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
