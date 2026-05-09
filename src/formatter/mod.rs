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
                let is_closing = raw.contains("{% end");
                let is_self_closing = raw.contains("{% extends") || raw.contains("{% include") || raw.contains("{% load") || raw.contains("{% static");

                if is_closing {
                    indent_level = indent_level.saturating_sub(1);
                }

                output.push_str(&" ".repeat(indent_level * config.indent));
                output.push_str(raw);
                output.push_str("\n");

                if !is_closing && !is_self_closing {
                    indent_level += 1;
                }
            }
        }
        i += 1;
    }

    output
}
