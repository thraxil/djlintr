use crate::config::Config;
use crate::formatter::tokenizer::{Token, Tokenizer};
use serde::{Deserialize, Serialize};
use regex::Regex;
use std::collections::HashSet;

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct LintError {
    pub code: String,
    pub line: usize,
    pub column: usize,
    pub match_str: String,
    pub message: String,
}

pub fn lint(config: &Config, source: &str) -> Vec<LintError> {
    let mut errors = Vec::new();
    let tokens: Vec<Token> = Tokenizer::new(source).collect();
    
    let mut open_tags: Vec<(String, usize, usize)> = Vec::new();
    let single_quote_attr_re = Regex::new(r#"\s+[a-zA-Z0-9:-]+='[^']*'"#).unwrap();
    let uppercase_attr_re = Regex::new(r#"\b[A-Z0-9:-]+="#).unwrap();
    
    // Batch 1 Rules Regex
    let unquoted_attr_re = Regex::new(r#"(?i)\s+(?:class|id|src|width|height|alt|style|lang|title|href|action|method|checked|required|srcset)=[^"'{>][^\s>]*"#).unwrap();
    let space_around_eq_re = Regex::new(r#"\b[a-zA-Z0-9:-]+\s+=|=\s+["'{a-zA-Z0-9]"#).unwrap();
    let js_link_re = Regex::new(r#"(?i)(?:href|action|data-url)=['"]javascript:"#).unwrap();
    let inline_style_re = Regex::new(r#"(?i)\bstyle=["']"#).unwrap();
    let http_link_re = Regex::new(r#"(?i)(?:href|src|action|data-url)=['"]http://"#).unwrap();
    let script_style_type_re = Regex::new(r#"(?i)\btype=['"](?:text/css|text/javascript)['"]"#).unwrap();
    let empty_id_class_re = Regex::new(r#"(?i)\b(?:id|class)=['"]['"]"#).unwrap();

    // Batch 2 State
    let mut has_doctype = false;
    let mut has_title = false;
    let mut has_meta_description = false;
    let mut has_meta_keywords = false;
    let mut html_tag_pos: Option<(usize, usize, String)> = None;
    let form_action_ws_re = Regex::new(r#"(?i)\baction=(?:\"\s+[^\"]*\s+\"|'\s+[^']*\s+')"#).unwrap();
    let attr_name_re = Regex::new(r#"(?i)\s([a-zA-Z0-9:-]+)="#).unwrap();

    // Batch 3 Regex
    let extra_blank_lines_re = Regex::new(r#"(?m)[^\n]{0,10}\n\s*\n\s*\n"#).unwrap();
    let spaceless_tags_re = Regex::new(r#"(?i)\b(?:class|id)=['"]\{%\s+(?:if|for).*?%\}.*?['"]"#).unwrap();
    let malformed_tag_re = Regex::new(r#"\{%[^}]*?\}%"#).unwrap();

    // Run whole-source regexes (like extra blank lines)
    for cap in extra_blank_lines_re.captures_iter(source) {
        errors.push(LintError {
            code: "H014".to_string(),
            line: 1, // Approximated
            column: 0,
            match_str: cap.get(0).unwrap().as_str().to_string(),
            message: "Found extra blank lines.".to_string(),
        });
    }

    let mut i = 0;
    while i < tokens.len() {
        let token = &tokens[i];
        match token {
            Token::Doctype { .. } => {
                has_doctype = true;
            }
            Token::Tag { name, raw, is_closing, is_self_closing, line, column } => {
                let name_lower = name.to_lowercase();
                let raw_lower = raw.to_lowercase();

                if *is_closing {
                    if let Some(last_open) = open_tags.pop() {
                        if &last_open.0 != name {
                            // Mismatched tags
                        }
                    } else {
                        errors.push(LintError {
                            code: "H025".to_string(),
                            line: *line,
                            column: *column,
                            match_str: raw.to_string(),
                            message: "Tag seems to be an orphan.".to_string(),
                        });
                    }

                    // Rule H015: Follow h tags with a line break
                    if name_lower.starts_with('h') && name_lower.len() == 2 && name_lower.chars().nth(1).unwrap().is_ascii_digit() {
                        if i + 1 < tokens.len() {
                            if let Token::Tag { line: next_line, raw: next_raw, .. } = &tokens[i+1] {
                                if line == next_line {
                                    errors.push(LintError {
                                        code: "H015".to_string(),
                                        line: *line,
                                        column: *column + raw.len(), // approximate column
                                        match_str: next_raw.to_string(),
                                        message: "Follow h tags with a line break.".to_string(),
                                    });
                                }
                            }
                        }
                    }

                } else {
                    if name_lower == "html" {
                        html_tag_pos = Some((*line, *column, raw.to_string()));
                        // Rule H007: DOCTYPE before html
                        if !has_doctype {
                            errors.push(LintError {
                                code: "H007".to_string(),
                                line: *line,
                                column: *column,
                                match_str: raw.to_string(),
                                message: "<!DOCTYPE ... > should be present before the html tag.".to_string(),
                            });
                        }
                    }

                    if name_lower == "title" {
                        has_title = true;
                    }
                    if name_lower == "meta" {
                        if raw_lower.contains("name=\"description\"") || raw_lower.contains("name='description'") {
                            has_meta_description = true;
                        }
                        if raw_lower.contains("name=\"keywords\"") || raw_lower.contains("name='keywords'") {
                            has_meta_keywords = true;
                        }
                        
                        // Rule H035: Meta tags should be self closing
                        if !raw.ends_with("/>") {
                            errors.push(LintError {
                                code: "H035".to_string(),
                                line: *line,
                                column: *column,
                                match_str: raw.to_string(),
                                message: "Meta tags should be self closing.".to_string(),
                            });
                        }
                    }

                    // Rule H017: Void tags self closing (excluding meta for H035)
                    if is_void_element(&name_lower) && name_lower != "meta" && !raw.ends_with("/>") {
                        errors.push(LintError {
                            code: "H017".to_string(),
                            line: *line,
                            column: *column,
                            match_str: raw.to_string(),
                            message: "Void tags should be self closing.".to_string(),
                        });
                    }

                    // Rule H020: Empty tag pair
                    if !is_self_closing && !is_void_element(&name_lower) && i + 1 < tokens.len() {
                        if let Token::Tag { is_closing: true, name: next_name, .. } = &tokens[i+1] {
                            if &next_name.to_lowercase() == &name_lower {
                                errors.push(LintError {
                                    code: "H020".to_string(),
                                    line: *line,
                                    column: *column,
                                    match_str: raw.to_string(),
                                    message: "Empty tag pair found. Consider removing.".to_string(),
                                });
                            }
                        }
                    }

                    // Rule H033: Form action whitespace
                    if name_lower == "form" && form_action_ws_re.is_match(raw) {
                        errors.push(LintError {
                            code: "H033".to_string(),
                            line: *line,
                            column: *column,
                            match_str: raw.to_string(),
                            message: "Extra whitespace found in form action.".to_string(),
                        });
                    }

                    // Rule H037: Duplicate attribute
                    let mut seen_attrs = HashSet::new();
                    for cap in attr_name_re.captures_iter(raw) {
                        let attr_name = cap.get(1).unwrap().as_str().to_lowercase();
                        if !seen_attrs.insert(attr_name) {
                            errors.push(LintError {
                                code: "H037".to_string(),
                                line: *line,
                                column: *column,
                                match_str: raw.to_string(),
                                message: "Duplicate attribute found.".to_string(),
                            });
                            break; // report once per tag to avoid spam
                        }
                    }

                    // Rule T028: Consider using spaceless tags inside attribute values
                    if let Some(m) = spaceless_tags_re.find(raw) {
                        errors.push(LintError {
                            code: "T028".to_string(),
                            line: *line,
                            column: *column,
                            match_str: m.as_str().to_string(),
                            message: "Consider using spaceless tags inside attribute values. {%- if/for -%}".to_string(),
                        });
                    }

                    // Rule H009: Tag names should be lowercase
                    if name.chars().any(|c| c.is_uppercase()) {
                        errors.push(LintError {
                            code: "H009".to_string(),
                            line: *line,
                            column: *column,
                            match_str: raw.to_string(),
                            message: "Tag names should be lowercase.".to_string(),
                        });
                    }

                    // Rule H010: Attribute names should be lowercase
                    if let Some(m) = uppercase_attr_re.find(raw) {
                        errors.push(LintError {
                            code: "H010".to_string(),
                            line: *line,
                            column: *column,
                            match_str: m.as_str().to_string(),
                            message: "Attribute names should be lowercase.".to_string(),
                        });
                    }

                    // Rule H005: lang attribute on html tag
                    if name_lower == "html" && !raw_lower.contains("lang=") {
                        errors.push(LintError {
                            code: "H005".to_string(),
                            line: *line,
                            column: *column,
                            match_str: raw.to_string(),
                            message: "Html tag should have lang attribute.".to_string(),
                        });
                    }

                    // Rule H006 & H013: img height, width, and alt
                    if name_lower == "img" {
                        if !raw_lower.contains("height=") || !raw_lower.contains("width=") {
                            errors.push(LintError {
                                code: "H006".to_string(),
                                line: *line,
                                column: *column,
                                match_str: raw.to_string(),
                                message: "Img tag should have height and width attributes.".to_string(),
                            });
                        }
                        if !raw_lower.contains("alt=") {
                            errors.push(LintError {
                                code: "H013".to_string(),
                                line: *line,
                                column: *column,
                                match_str: raw.to_string(),
                                message: "Img tag should have an alt attribute.".to_string(),
                            });
                        }
                    }

                    // Rule H008: Attributes should be double quoted
                    if let Some(m) = single_quote_attr_re.find(raw) {
                        errors.push(LintError {
                            code: "H008".to_string(),
                            line: *line,
                            column: *column,
                            match_str: raw[..m.end()].to_string(),
                            message: "Attributes should be double quoted.".to_string(),
                        });
                    }
                    
                    // Rule H011: Attribute values should be quoted
                    if let Some(m) = unquoted_attr_re.find(raw) {
                        errors.push(LintError {
                            code: "H011".to_string(),
                            line: *line,
                            column: *column,
                            match_str: m.as_str().to_string(),
                            message: "Attribute values should be quoted.".to_string(),
                        });
                    }

                    // Rule H012: There should be no spaces around attribute =
                    if let Some(m) = space_around_eq_re.find(raw) {
                        errors.push(LintError {
                            code: "H012".to_string(),
                            line: *line,
                            column: *column,
                            match_str: m.as_str().to_string(),
                            message: "There should be no spaces around attribute =.".to_string(),
                        });
                    }

                    // Rule H019: Replace 'javascript:abc()'
                    if let Some(m) = js_link_re.find(raw) {
                        errors.push(LintError {
                            code: "H019".to_string(),
                            line: *line,
                            column: *column,
                            match_str: m.as_str().to_string(),
                            message: "Replace 'javascript:abc()' with on_ event and real url.".to_string(),
                        });
                    }

                    // Rule H021: Inline styles should be avoided
                    if let Some(m) = inline_style_re.find(raw) {
                        errors.push(LintError {
                            code: "H021".to_string(),
                            line: *line,
                            column: *column,
                            match_str: m.as_str().to_string(),
                            message: "Inline styles should be avoided.".to_string(),
                        });
                    }

                    // Rule H022: Use HTTPS for external links
                    if let Some(m) = http_link_re.find(raw) {
                        errors.push(LintError {
                            code: "H022".to_string(),
                            line: *line,
                            column: *column,
                            match_str: m.as_str().to_string(),
                            message: "Use HTTPS for external links.".to_string(),
                        });
                    }

                    // Rule H024: Omit type on scripts and styles
                    if (name_lower == "script" || name_lower == "style") && script_style_type_re.is_match(raw) {
                        errors.push(LintError {
                            code: "H024".to_string(),
                            line: *line,
                            column: *column,
                            match_str: raw.to_string(),
                            message: "Omit type on scripts and styles.".to_string(),
                        });
                    }

                    // Rule H026: Empty id and class tags can be removed
                    if let Some(m) = empty_id_class_re.find(raw) {
                        errors.push(LintError {
                            code: "H026".to_string(),
                            line: *line,
                            column: *column,
                            match_str: m.as_str().to_string(),
                            message: "Empty id and class tags can be removed.".to_string(),
                        });
                    }

                    // Rule H029: Consider using lowercase form method values
                    if name_lower == "form" {
                        if let Some(caps) = Regex::new(r#"(?i)method=['"]([A-Z0-9]+)['"]"#).unwrap().captures(raw) {
                            let method_val = caps.get(1).unwrap().as_str();
                            if method_val.chars().any(|c| c.is_uppercase()) {
                                errors.push(LintError {
                                    code: "H029".to_string(),
                                    line: *line,
                                    column: *column,
                                    match_str: caps.get(0).unwrap().as_str().to_string(),
                                    message: "Consider using lowercase form method values.".to_string(),
                                });
                            }
                        }
                    }

                    // Rule H036: Avoid use of <br> tags
                    if name_lower == "br" {
                        errors.push(LintError {
                            code: "H036".to_string(),
                            line: *line,
                            column: *column,
                            match_str: raw.to_string(),
                            message: "Avoid use of <br> tags.".to_string(),
                        });
                    }

                    // Rule D004 & J004: Static urls
                    if name_lower == "link" || name_lower == "script" || name_lower == "img" {
                        if raw.contains("src=\"/static/") || raw.contains("href=\"/static/") {
                            errors.push(LintError {
                                code: "D004".to_string(),
                                line: *line,
                                column: *column,
                                match_str: raw.to_string(),
                                message: "(Django) Static urls should follow {% static path/to/file %} pattern.".to_string(),
                            });
                            errors.push(LintError {
                                code: "J004".to_string(),
                                line: *line,
                                column: *column,
                                match_str: raw.to_string(),
                                message: "(Jinja) Static urls should follow {{ url_for('static'..) }} pattern.".to_string(),
                            });
                        }
                    }

                    // Rule D018 & J018: Internal links
                    if name_lower == "a" || name_lower == "form" {
                        if (raw.contains("href=\"/") || raw.contains("action=\"/")) 
                           && !raw.contains("href=\"#") && !raw.contains("action=\"#")
                           && !raw.contains("{% url") {
                            errors.push(LintError {
                                code: "D018".to_string(),
                                line: *line,
                                column: *column,
                                match_str: raw.to_string(),
                                message: "(Django) Internal links should use the {% url ... %} pattern.".to_string(),
                            });
                            errors.push(LintError {
                                code: "J018".to_string(),
                                line: *line,
                                column: *column,
                                match_str: raw.to_string(),
                                message: "(Jinja) Internal links should use the {{ url_for() ... }} pattern.".to_string(),
                            });
                        }
                    }

                    if !is_self_closing {
                        open_tags.push((name_lower, *line, *column));
                    }
                }
            }
            Token::DjangoVar { raw, line, column } | Token::DjangoBlock { raw, line, column } => {
                let is_var = matches!(token, Token::DjangoVar { .. });
                let (open_tag, close_tag) = if is_var { ("{{", "}}") } else { ("{%", "%}") };

                if !raw.starts_with(&format!("{} ", open_tag)) || !raw.ends_with(&format!(" {}", close_tag)) {
                    errors.push(LintError {
                        code: "T001".to_string(),
                        line: *line,
                        column: *column,
                        match_str: raw.to_string(),
                        message: "Variables should be wrapped in a whitespace.".to_string(),
                    });
                }

                if !is_var {
                    if raw.contains('\'') && (raw.contains("extends") || raw.contains("include") || raw.contains("with") || raw.contains("trans") || raw.contains("now")) {
                        errors.push(LintError {
                            code: "T002".to_string(),
                            line: *line,
                            column: *column,
                            match_str: raw.to_string(),
                            message: "Double quotes should be used in tags.".to_string(),
                        });
                    }

                    let inner = raw.trim_start_matches("{%").trim_end_matches("%}").trim();
                    if inner == "endblock" {
                        errors.push(LintError {
                            code: "T003".to_string(),
                            line: *line,
                            column: *column,
                            match_str: raw.to_string(),
                            message: "Endblock should have name. Ex: {% endblock body %}.".to_string(),
                        });
                    }
                }

                // Rule T027: Unclosed string
                // Very simple heuristic: count single and double quotes
                let single_quotes = raw.chars().filter(|&c| c == '\'').count();
                let double_quotes = raw.chars().filter(|&c| c == '"').count();
                if single_quotes % 2 != 0 || double_quotes % 2 != 0 {
                    errors.push(LintError {
                        code: "T027".to_string(),
                        line: *line,
                        column: *column,
                        match_str: raw.to_string(),
                        message: "Unclosed string found in template syntax.".to_string(),
                    });
                }

                // Rule T034: Malformed tag
                if let Some(m) = malformed_tag_re.find(raw) {
                    errors.push(LintError {
                        code: "T034".to_string(),
                        line: *line,
                        column: *column,
                        match_str: m.as_str().to_string(),
                        message: "Did you intend to use {% ... %} instead of {% ... }%?".to_string(),
                    });
                }
            }
            Token::Text { raw, line, column } => {
                // Rule H023: Do not use entity references
                if let Some(m) = Regex::new(r#"&(?:[a-zA-Z0-9]+|#[0-9]+|#x[0-9a-fA-F]+);"#).unwrap().find(raw) {
                    let entity = m.as_str();
                    // djlint allows some common ones like &nbsp;, &lt;, &gt;, &amp;, &quot;, &ensp;, &emsp;, &thinsp;, &shy;
                    if !matches!(entity, "&nbsp;" | "&lt;" | "&gt;" | "&amp;" | "&quot;" | "&ensp;" | "&emsp;" | "&thinsp;" | "&shy;") {
                        errors.push(LintError {
                            code: "H023".to_string(),
                            line: *line,
                            column: *column,
                            match_str: entity.to_string(),
                            message: "Do not use entity references.".to_string(),
                        });
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }

    // Document-level checks for Batch 2
    if let Some((line, column, match_str)) = html_tag_pos {
        if !has_title {
            errors.push(LintError {
                code: "H016".to_string(),
                line,
                column,
                match_str: match_str.clone(),
                message: "Missing title tag in html.".to_string(),
            });
        }
        if !has_meta_description {
            errors.push(LintError {
                code: "H030".to_string(),
                line,
                column,
                match_str: match_str.clone(),
                message: "Consider adding a meta description.".to_string(),
            });
        }
        if !has_meta_keywords {
            errors.push(LintError {
                code: "H031".to_string(),
                line,
                column,
                match_str: match_str.clone(),
                message: "Consider adding meta keywords.".to_string(),
            });
        }
    }

    // After all tokens, if any open_tags left, they are orphans
    if source.to_lowercase().contains("<a>") || source.to_lowercase().contains("<html>") || source.to_lowercase().contains("<div>") {
        for (tag_name, line, column) in open_tags {
            errors.push(LintError {
                code: "H025".to_string(),
                line,
                column,
                match_str: format!("<{}>", tag_name),
                message: "Tag seems to be an orphan.".to_string(),
            });
        }
    }

    errors.sort_by_key(|e| (e.line, e.column));
    
    // Filter ignored rules
    errors.into_iter().filter(|e| !config.ignore.contains(&e.code)).collect()
}

fn is_void_element(name: &str) -> bool {
    matches!(
        name,
        "area" | "base" | "br" | "col" | "embed" | "hr" | "img" | "input" | "link" | "meta" | "param" | "source" | "track" | "wbr"
    )
}
