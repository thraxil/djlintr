pub mod tokenizer;

use crate::config::Config;
use regex::Regex;
use std::sync::OnceLock;
use tokenizer::{Token, Tokenizer};

fn is_inline_tag(name: &str) -> bool {
    let inline_tags = [
        "a", "abbr", "acronym", "b", "bdo", "big", "cite", "code", "dfn", "em", "i", "img", "kbd",
        "map", "object", "q", "samp", "small", "span", "strong", "sub", "sup", "tt", "var",
        "title", "option", "script", "style", "time",
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
        "script",
        "style",
    ];
    !no_indent_tags.contains(&name.to_lowercase().as_str())
}

fn should_wrap_attributes(name: &str) -> bool {
    let no_wrap_tags = [
        "path", "circle", "rect", "line", "polyline", "polygon", "ellipse",
    ];
    !no_wrap_tags.contains(&name.to_lowercase().as_str())
}

struct Formatter<'a> {
    config: &'a Config,
    output: String,
    indent_level: usize,
    tokens: Vec<Token<'a>>,
    pos: usize,
    formatting_enabled: bool,
    verbatim_tags: Vec<String>,
    at_start_of_line: bool,
    parent_stack: Vec<(usize, bool)>,
}

impl<'a> Formatter<'a> {
    fn new(config: &'a Config, source: &'a str) -> Self {
        let tokens: Vec<Token> = Tokenizer::new(source).collect();
        Self {
            config,
            output: String::new(),
            indent_level: 0,
            tokens,
            pos: 0,
            formatting_enabled: true,
            verbatim_tags: Vec::new(),
            at_start_of_line: true,
            parent_stack: Vec::new(),
        }
    }

    fn format(mut self) -> String {
        while self.pos < self.tokens.len() {
            self.process_token();
            self.pos += 1;
        }

        if !self.output.is_empty() && !self.output.ends_with('\n') {
            self.output.push('\n');
        }

        self.output
    }

    fn process_token(&mut self) {
        let token = &self.tokens[self.pos].clone();
        let raw = token.raw();

        if !self.formatting_enabled {
            if let Token::Comment { .. } | Token::DjangoBlock { .. } | Token::DjangoComment { .. } =
                token
            {
                if raw.contains("djlint:on") {
                    self.formatting_enabled = true;
                    trim_trailing_whitespace(&mut self.output);
                    self.output.push_str(raw.trim());
                    self.output.push('\n');
                    self.at_start_of_line = true;
                    return;
                }
            }
            self.output.push_str(raw);
            self.at_start_of_line = raw.ends_with('\n');
            return;
        }

        match token {
            Token::Doctype { .. } => self.handle_doctype(token),
            Token::Comment { .. } | Token::DjangoComment { .. } => self.handle_comment(token),
            Token::Tag { .. } => self.handle_tag(token),
            Token::Text { .. } => self.handle_text(token),
            Token::DjangoVar { .. } => self.handle_django_var(token),
            Token::DjangoBlock { .. } => self.handle_django_block(token),
        }
    }

    fn handle_doctype(&mut self, token: &Token) {
        let raw = token.raw();
        if !self.verbatim_tags.is_empty() {
            self.output.push_str(raw);
            self.at_start_of_line = raw.ends_with('\n');
        } else {
            self.output
                .push_str(&" ".repeat(self.indent_level * self.config.indent));
            self.output.push_str("<!DOCTYPE html>\n");
            self.at_start_of_line = true;
        }
    }

    fn handle_comment(&mut self, token: &Token) {
        let raw = token.raw();
        if !self.verbatim_tags.is_empty() {
            self.output.push_str(raw);
            self.at_start_of_line = raw.ends_with('\n');
        } else {
            let is_control = raw.contains("djlint:off") || raw.contains("djlint:on");
            if !is_control {
                if !self.at_start_of_line {
                    trim_trailing_whitespace(&mut self.output);
                    self.output.push('\n');
                }
                self.output
                    .push_str(&" ".repeat(self.indent_level * self.config.indent));
                self.output.push_str(raw.trim());
                self.output.push('\n');
                self.at_start_of_line = true;
            } else {
                self.output.push_str(raw.trim());
                if raw.contains("djlint:off") {
                    self.formatting_enabled = false;
                    self.at_start_of_line = false;
                } else {
                    self.output.push('\n');
                    self.at_start_of_line = true;
                }
            }
        }
    }

    fn handle_tag(&mut self, token: &Token) {
        if let Token::Tag {
            name,
            raw,
            is_closing,
            is_self_closing,
            ..
        } = token
        {
            let name_lower = name.to_lowercase();

            if !self.verbatim_tags.is_empty() {
                if *is_closing && self.verbatim_tags.last() == Some(&name_lower) {
                    // Closing verbatim tag
                    let was_verbatim_name = self.verbatim_tags.pop();
                    let is_closing_verbatim = was_verbatim_name.is_some();

                    let mut incremented = false;
                    if let Some((_, inc)) = self.parent_stack.pop() {
                        incremented = inc;
                    }
                    if incremented {
                        self.indent_level = self.indent_level.saturating_sub(1);
                    }

                    if !self.at_start_of_line && !is_inline_tag(name) && !is_closing_verbatim {
                        trim_trailing_whitespace(&mut self.output);
                        self.output.push('\n');
                        self.at_start_of_line = true;
                    }
                    if self.at_start_of_line {
                        self.output
                            .push_str(&" ".repeat(self.indent_level * self.config.indent));
                    }
                    self.output.push_str(&format!("</{}>", name));

                    let mut should_newline = true;
                    if (is_inline_tag(name) || is_closing_verbatim)
                        && self.pos + 1 < self.tokens.len()
                    {
                        let next_token = &self.tokens[self.pos + 1];
                        if next_token.line() == token.ends_on_line()
                            && !token.raw().ends_with('\n')
                            && !token.raw().ends_with("\r\n")
                            && is_inline_ish(next_token, self.config)
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
                        self.output.push('\n');
                        self.at_start_of_line = true;
                    } else {
                        self.at_start_of_line = false;
                    }
                    return;
                } else {
                    self.output.push_str(raw);
                    self.at_start_of_line = raw.ends_with('\n');
                    return;
                }
            }

            if *is_closing {
                let mut incremented = false;
                if let Some((_, inc)) = self.parent_stack.pop() {
                    incremented = inc;
                }
                if incremented {
                    self.indent_level = self.indent_level.saturating_sub(1);
                }

                if !self.at_start_of_line && !is_inline_tag(name) {
                    trim_trailing_whitespace(&mut self.output);
                    self.output.push('\n');
                    self.at_start_of_line = true;
                }
                if self.at_start_of_line {
                    self.output
                        .push_str(&" ".repeat(self.indent_level * self.config.indent));
                }
                self.output.push_str(&format!("</{}>", name));

                let mut should_newline = true;
                if is_inline_tag(name) && self.pos + 1 < self.tokens.len() {
                    let next_token = &self.tokens[self.pos + 1];
                    if next_token.line() == token.ends_on_line()
                        && !token.raw().ends_with('\n')
                        && !token.raw().ends_with("\r\n")
                        && is_inline_ish(next_token, self.config)
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
                    self.output.push('\n');
                    self.at_start_of_line = true;
                } else {
                    self.at_start_of_line = false;
                }
            } else {
                let (children, closing_idx) = get_children_info(self.pos, &self.tokens);

                let is_potentially_verbatim =
                    matches!(name_lower.as_str(), "style" | "script") && !is_self_closing;

                if !self.at_start_of_line && (is_html_block_tag(name) || is_potentially_verbatim) {
                    trim_trailing_whitespace(&mut self.output);
                    self.output.push('\n');
                    self.at_start_of_line = true;
                }

                let started_on_newline = self.at_start_of_line;
                if self.at_start_of_line {
                    self.output
                        .push_str(&" ".repeat(self.indent_level * self.config.indent));
                }

                let formatted_tag =
                    format_tag(name, raw, *is_self_closing, self.indent_level, self.config);
                self.output.push_str(&formatted_tag);
                self.at_start_of_line =
                    formatted_tag.ends_with('\n') || formatted_tag.ends_with("\r\n");

                // Check if we can inline
                let is_block_parent = is_html_block_tag(name);
                let is_structural = is_structural_tag(name);
                let mut did_collapse = false;
                if !is_self_closing {
                    if let Some(j) = closing_idx {
                        let logical_elements = get_logical_elements(&children, &self.tokens);

                        let all_inline_ish = logical_elements.iter().all(|range| {
                            if range.len() == 1 {
                                is_strictly_inline(&self.tokens[range.start], self.config)
                            } else {
                                // It's a tag pair. Inline if it's an inline tag and its content is inlinable.
                                if let Token::Tag { name: n, .. } = &self.tokens[range.start] {
                                    is_inline_tag(n)
                                        && is_tag_range_inlinable(range, &self.tokens, self.config)
                                } else {
                                    false
                                }
                            }
                        });

                        let has_any_tag = logical_elements.iter().any(|range| {
                            if range.len() > 1 {
                                true
                            } else {
                                let token = &self.tokens[range.start];
                                matches!(
                                    token,
                                    Token::Tag { .. }
                                        | Token::Comment { .. }
                                        | Token::DjangoComment { .. }
                                )
                            }
                        });

                        let has_newline_text = logical_elements.iter().any(|range| {
                            if range.len() == 1 {
                                if let Token::Text { raw, .. } = &self.tokens[range.start] {
                                    raw.contains('\n')
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        });

                        let non_whitespace_elements: Vec<_> = logical_elements
                            .iter()
                            .filter(|range| {
                                if range.len() == 1 {
                                    if let Token::Text { raw, .. } = &self.tokens[range.start] {
                                        !raw.trim().is_empty()
                                    } else {
                                        true
                                    }
                                } else {
                                    true
                                }
                            })
                            .collect();

                        if all_inline_ish {
                            let can_collapse = if is_structural {
                                false
                            } else if is_block_parent {
                                if logical_elements.is_empty() {
                                    // Empty tag. Collapse only if j follows i and is on same line
                                    j == self.pos + 1
                                        && self.tokens[j].line() == token.ends_on_line()
                                } else {
                                    // Not empty. Check if all elements are on the same line as the start and end tags.
                                    let mut same_line = self.tokens[j].line() == token.line();
                                    if same_line {
                                        for range in &logical_elements {
                                            if self.tokens[range.start].line() != token.line()
                                                || self.tokens[range.end - 1].ends_on_line()
                                                    != token.line()
                                            {
                                                same_line = false;
                                                break;
                                            }
                                        }
                                    }
                                    ((same_line && !has_newline_text)
                                        || non_whitespace_elements.len() == 1)
                                        && !has_any_tag
                                }
                            } else if has_any_tag {
                                !has_newline_text
                            } else {
                                true
                            };

                            if can_collapse {
                                let is_wrapped = formatted_tag.contains('\n');
                                if !is_wrapped || !has_any_tag {
                                    // Collapse
                                    let content = format_range_inlined_joined(
                                        &logical_elements,
                                        &self.tokens,
                                        self.indent_level,
                                        self.config,
                                    );
                                    let collapsed_content = content.trim();
                                    let tag_last_line_len =
                                        formatted_tag.split('\n').next_back().unwrap_or("").len();
                                    let current_line_len = if formatted_tag.contains('\n') {
                                        tag_last_line_len
                                    } else {
                                        (self.indent_level * self.config.indent) + tag_last_line_len
                                    };
                                    let projected_len =
                                        current_line_len + collapsed_content.len() + name.len() + 3;

                                    if (projected_len <= self.config.max_line_length
                                        || is_potentially_verbatim)
                                        && (logical_elements.is_empty()
                                            && j == self.pos + 1
                                            && self.tokens[j].line() == token.ends_on_line()
                                            || !logical_elements.is_empty())
                                    {
                                        self.output.push_str(collapsed_content);
                                        self.output.push_str(&format!("</{}>", name));
                                        let mut should_newline = true;
                                        if is_inline_tag(name) && j + 1 < self.tokens.len() {
                                            let next_token = &self.tokens[j + 1];
                                            if next_token.line() == token.ends_on_line()
                                                && !token.raw().ends_with('\n')
                                                && !token.raw().ends_with("\r\n")
                                                && is_inline_ish(next_token, self.config)
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
                                            trim_trailing_whitespace(&mut self.output);
                                            self.output.push('\n');
                                            self.at_start_of_line = true;
                                        } else {
                                            self.at_start_of_line = false;
                                        }
                                        self.pos = j;
                                        did_collapse = true;
                                    }
                                }
                            }

                            if !did_collapse
                                && !is_structural
                                && !logical_elements.is_empty()
                                && !is_block_parent
                                && (!has_any_tag || !has_newline_text)
                                && !is_potentially_verbatim
                            {
                                let child_indent = if is_inline_tag(name) {
                                    self.indent_level
                                } else {
                                    self.indent_level + 1
                                };
                                let content = format_range_inlined_joined(
                                    &logical_elements,
                                    &self.tokens,
                                    child_indent,
                                    self.config,
                                );
                                let collapsed_content = content.trim();

                                trim_trailing_whitespace(&mut self.output);
                                self.output.push('\n');
                                self.output
                                    .push_str(&" ".repeat(child_indent * self.config.indent));
                                self.output.push_str(collapsed_content);
                                trim_trailing_whitespace(&mut self.output);
                                self.output.push('\n');
                                self.output
                                    .push_str(&" ".repeat(self.indent_level * self.config.indent));
                                self.output.push_str(&format!("</{}>", name));
                                let mut should_newline = true;
                                if is_inline_tag(name) && j + 1 < self.tokens.len() {
                                    let next_token = &self.tokens[j + 1];
                                    if next_token.line() == token.ends_on_line()
                                        && !token.raw().ends_with('\n')
                                        && !token.raw().ends_with("\r\n")
                                        && is_inline_ish(next_token, self.config)
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
                                    trim_trailing_whitespace(&mut self.output);
                                    self.output.push('\n');
                                    self.at_start_of_line = true;
                                } else {
                                    self.at_start_of_line = false;
                                }
                                self.pos = j;
                                did_collapse = true;
                            }
                        }
                    }
                }

                if !did_collapse {
                    let mut is_verbatim = false;
                    if is_potentially_verbatim {
                        is_verbatim = true;
                        self.verbatim_tags.push(name_lower.clone());
                    }

                    let mut should_newline = false;
                    if !is_verbatim {
                        should_newline = true;
                        if !is_structural && self.pos + 1 < self.tokens.len() {
                            let next_token = &self.tokens[self.pos + 1];
                            if next_token.line() == token.ends_on_line()
                                && !token.raw().ends_with('\n')
                                && !token.raw().ends_with("\r\n")
                                && is_inline_ish(next_token, self.config)
                                && (is_inline_tag(name) || !is_html_block_tag(name))
                            {
                                should_newline = false;
                            }
                        }
                        if should_newline {
                            trim_trailing_whitespace(&mut self.output);
                            self.output.push('\n');
                            self.at_start_of_line = true;
                        } else {
                            self.at_start_of_line = false;
                        }
                    } else {
                        self.at_start_of_line =
                            formatted_tag.ends_with('\n') || formatted_tag.ends_with("\r\n");
                    }

                    let mut incremented = false;
                    let mut will_start_newline = should_newline;
                    if !will_start_newline && self.pos + 1 < self.tokens.len() {
                        if let Token::Text { raw: r, .. } = &self.tokens[self.pos + 1] {
                            if r.starts_with('\n') || r.starts_with("\r\n") {
                                will_start_newline = true;
                            }
                        }
                    }

                    let has_newline_in_children = children
                        .iter()
                        .any(|&idx| self.tokens[idx].raw().contains('\n'));

                    if !is_self_closing
                        && should_indent_children(name)
                        && (!is_inline_tag(name) || started_on_newline)
                        && (will_start_newline || has_newline_in_children)
                        && !is_verbatim
                    {
                        self.indent_level += 1;
                        incremented = true;
                    }
                    if !is_self_closing {
                        self.parent_stack.push((self.pos, incremented));
                    }
                }
            }
        }
    }

    fn handle_text(&mut self, token: &Token) {
        let raw = token.raw();
        if !self.verbatim_tags.is_empty() {
            self.output.push_str(raw);
            self.at_start_of_line = raw.ends_with('\n') || raw.ends_with("\r\n");
        } else {
            let trimmed = raw.trim();
            if !trimmed.is_empty() {
                let lines: Vec<&str> = trimmed.lines().collect();
                let mut blank_lines = 0;
                for (idx, line) in lines.iter().enumerate() {
                    let is_last_line = idx == lines.len() - 1;

                    if line.trim().is_empty() {
                        blank_lines += 1;
                        if blank_lines <= self.config.max_blank_lines {
                            trim_trailing_whitespace(&mut self.output);
                            self.output.push('\n');
                            self.at_start_of_line = true;
                        }
                        continue;
                    }
                    blank_lines = 0;

                    if self.at_start_of_line {
                        self.output
                            .push_str(&" ".repeat(self.indent_level * self.config.indent));
                        self.output.push_str(line.trim_start());
                    } else if idx == 0 {
                        if raw.starts_with('\n') || raw.starts_with("\r\n") {
                            trim_trailing_whitespace(&mut self.output);
                            self.output.push('\n');
                            self.output
                                .push_str(&" ".repeat(self.indent_level * self.config.indent));
                            self.output.push_str(line.trim_start());
                        } else {
                            // Continuing inline. We want to preserve original leading spaces
                            let leading_spaces = raw.chars().take_while(|&c| c == ' ').count();
                            if leading_spaces > 0 {
                                self.output.push_str(&" ".repeat(leading_spaces));
                            }
                            self.output.push_str(line);
                        }
                    } else {
                        trim_trailing_whitespace(&mut self.output);
                        self.output.push('\n');
                        self.output
                            .push_str(&" ".repeat(self.indent_level * self.config.indent));
                        self.output.push_str(line.trim_start());
                    }

                    if is_last_line {
                        let mut should_newline = true;
                        if self.pos + 1 < self.tokens.len() {
                            let next_token = &self.tokens[self.pos + 1];
                            if next_token.line() == token.ends_on_line()
                                && !token.raw().ends_with('\n')
                                && !token.raw().ends_with("\r\n")
                                && is_inline_ish(next_token, self.config)
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
                            trim_trailing_whitespace(&mut self.output);
                            self.output.push('\n');
                            self.at_start_of_line = true;
                        } else {
                            // Preserve original trailing space if any
                            let trailing_spaces =
                                raw.chars().rev().take_while(|&c| c == ' ').count();
                            if trailing_spaces > 0 {
                                self.output.push_str(&" ".repeat(trailing_spaces));
                            }
                            self.at_start_of_line = false;
                        }
                    } else {
                        trim_trailing_whitespace(&mut self.output);
                        self.output.push('\n');
                        self.at_start_of_line = true;
                    }
                }
            } else if !raw.is_empty() {
                if raw.contains('\n') {
                    if !self.at_start_of_line {
                        trim_trailing_whitespace(&mut self.output);
                        self.output.push('\n');
                    }
                    self.at_start_of_line = true;
                } else if !self.at_start_of_line {
                    self.output.push_str(raw);
                }
            }
        }
    }

    fn handle_django_var(&mut self, token: &Token) {
        let raw = token.raw();
        if !self.verbatim_tags.is_empty() {
            self.output.push_str(raw);
            self.at_start_of_line = raw.ends_with('\n');
        } else {
            if self.at_start_of_line {
                self.output
                    .push_str(&" ".repeat(self.indent_level * self.config.indent));
            }
            self.output.push_str(&normalize_django(raw));

            let mut should_newline = true;
            if self.pos + 1 < self.tokens.len() {
                let next_token = &self.tokens[self.pos + 1];
                if next_token.line() == token.ends_on_line()
                    && !token.raw().ends_with('\n')
                    && !token.raw().ends_with("\r\n")
                    && is_inline_ish(next_token, self.config)
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
                trim_trailing_whitespace(&mut self.output);
                self.output.push('\n');
                self.at_start_of_line = true;
            } else {
                self.at_start_of_line = false;
            }
        }
    }

    fn handle_django_block(&mut self, token: &Token) {
        let raw = token.raw();
        if !self.verbatim_tags.is_empty() {
            self.output.push_str(raw);
            self.at_start_of_line = raw.ends_with('\n');
        } else {
            let tag_name = get_django_tag_name(raw).unwrap_or("");
            let is_closing = tag_name.starts_with("end");
            let actual_tag_name = if let Some(stripped) = tag_name.strip_prefix("end") {
                stripped
            } else {
                tag_name
            };
            let is_block = is_block_tag(actual_tag_name, &self.config.custom_blocks);
            let is_reindent = is_reindent_tag(tag_name);

            if (is_closing && is_block) || is_reindent {
                let mut incremented = false;
                if let Some((_, inc)) = self.parent_stack.pop() {
                    incremented = inc;
                }
                if incremented {
                    self.indent_level = self.indent_level.saturating_sub(1);
                }
            }

            // Check if we can inline
            let mut did_collapse = false;
            if !is_closing && is_block {
                let (children, closing_idx) = get_children_info(self.pos, &self.tokens);

                if let Some(j) = closing_idx {
                    let logical_elements = get_logical_elements(&children, &self.tokens);

                    let all_inline_ish = logical_elements.iter().all(|range| {
                        if range.len() == 1 {
                            is_strictly_inline(&self.tokens[range.start], self.config)
                        } else {
                            // It's a tag pair.
                            let first_token = &self.tokens[range.start];
                            match first_token {
                                Token::Tag { name: n, .. } => {
                                    let is_block = is_html_block_tag(n);
                                    if is_block {
                                        // Don't inline block tags if they contain other tags
                                        let children_indices: Vec<usize> =
                                            (range.start + 1..range.end - 1).collect();
                                        let sub_elements =
                                            get_logical_elements(&children_indices, &self.tokens);
                                        let has_sub_tag = sub_elements.iter().any(|r| r.len() > 1);
                                        !has_sub_tag
                                            && is_tag_range_inlinable(
                                                range,
                                                &self.tokens,
                                                self.config,
                                            )
                                    } else {
                                        is_inline_tag(n)
                                            && is_tag_range_inlinable(
                                                range,
                                                &self.tokens,
                                                self.config,
                                            )
                                    }
                                }
                                Token::DjangoBlock { .. } => {
                                    is_tag_range_inlinable(range, &self.tokens, self.config)
                                }
                                _ => false,
                            }
                        }
                    });

                    if all_inline_ish {
                        let mut same_line = self.tokens[j].line() == token.line();
                        if same_line {
                            for range in &logical_elements {
                                if self.tokens[range.start].line() != token.line()
                                    || self.tokens[range.end - 1].ends_on_line() != token.line()
                                {
                                    same_line = false;
                                    break;
                                }
                            }
                        }

                        let non_whitespace_elements: Vec<_> = logical_elements
                            .iter()
                            .filter(|range| {
                                if range.len() == 1 {
                                    if let Token::Text { raw, .. } = &self.tokens[range.start] {
                                        !raw.trim().is_empty()
                                    } else {
                                        true
                                    }
                                } else {
                                    true
                                }
                            })
                            .collect();

                        let has_any_html_tag = non_whitespace_elements.iter().any(|range| {
                            if range.len() > 1 {
                                true
                            } else {
                                matches!(self.tokens[range.start], Token::Tag { .. })
                            }
                        });

                        let normalized_start = normalize_django(raw);
                        let normalized_end = normalize_django(self.tokens[j].raw());
                        let content = format_range_inlined_joined(
                            &logical_elements,
                            &self.tokens,
                            self.indent_level + 1,
                            self.config,
                        );
                        let collapsed_content = content.trim();
                        let projected_len = (self.indent_level * self.config.indent)
                            + normalized_start.len()
                            + collapsed_content.len()
                            + normalized_end.len();

                        if projected_len <= self.config.max_line_length
                            && (same_line
                                || !has_any_html_tag
                                || non_whitespace_elements.len() == 1)
                        {
                            if self.at_start_of_line {
                                self.output
                                    .push_str(&" ".repeat(self.indent_level * self.config.indent));
                            }
                            self.output.push_str(&normalized_start);
                            self.output.push_str(collapsed_content);
                            self.output.push_str(&normalized_end);
                            trim_trailing_whitespace(&mut self.output);
                            self.output.push('\n');
                            self.at_start_of_line = true;
                            self.pos = j;
                            did_collapse = true;
                        }
                    }
                }
            }

            if !did_collapse {
                if !self.at_start_of_line && (is_block || is_reindent) {
                    trim_trailing_whitespace(&mut self.output);
                    self.output.push('\n');
                    self.at_start_of_line = true;
                }

                if self.at_start_of_line {
                    self.output
                        .push_str(&" ".repeat(self.indent_level * self.config.indent));
                }
                self.output.push_str(&normalize_django(raw));

                if raw.contains("djlint:off") {
                    self.formatting_enabled = false;
                }

                let mut should_newline = true;
                if !is_block && !is_reindent && self.pos + 1 < self.tokens.len() {
                    let next_token = &self.tokens[self.pos + 1];
                    if next_token.line() == token.ends_on_line()
                        && !token.raw().ends_with('\n')
                        && !token.raw().ends_with("\r\n")
                        && is_inline_ish(next_token, self.config)
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
                    trim_trailing_whitespace(&mut self.output);
                    self.output.push('\n');
                    self.at_start_of_line = true;
                } else {
                    self.at_start_of_line = false;
                }

                if (!is_closing && is_block) || is_reindent {
                    let (_, closing_idx) = get_children_info(self.pos, &self.tokens);
                    if closing_idx.is_some() || is_reindent {
                        self.indent_level += 1;
                        self.parent_stack.push((self.pos, true));
                    }
                }
            }
        }
    }
}

pub fn format(config: &Config, source: &str) -> String {
    Formatter::new(config, source).format()
}

static VAR_RE: OnceLock<Regex> = OnceLock::new();
static BLOCK_RE: OnceLock<Regex> = OnceLock::new();

fn normalize_django(raw: &str) -> String {
    let var_re = VAR_RE.get_or_init(|| Regex::new(r#"\{\{[\s\S]*?\}\}"#).unwrap());
    let block_re = BLOCK_RE.get_or_init(|| Regex::new(r#"\{%[\s\S]*?%\}"#).unwrap());

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

static ATTR_RE: OnceLock<Regex> = OnceLock::new();
static STYLE_ATTR_RE: OnceLock<Regex> = OnceLock::new();

static WHITESPACE_RE: OnceLock<Regex> = OnceLock::new();

fn format_tag(
    name: &str,
    raw: &str,
    is_self_closing: bool,
    indent_level: usize,
    config: &Config,
) -> String {
    let attr_re = ATTR_RE.get_or_init(|| {
        Regex::new(
            r#"([a-zA-Z0-9:@._#*!-]+(?:\s*=\s*(?:"(?:\{\{[\s\S]*?\}\}|\{%[\s\S]*?%\}|[^"])*"|'(?:\{\{[\s\S]*?\}\}|\{%[\s\S]*?%\}|[^'])*'|[^\s>]+))?|\{\{[\s\S]*?\}\}|\{%[\s\S]*?%\})"#,
        )
        .unwrap()
    });
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

    let attrs: Vec<String> = attr_re
        .find_iter(content)
        .map(|m| normalize_django(m.as_str()))
        .collect();

    let mut final_content = String::new();
    let mut last_end = 0;
    let mut prev_was_django_block = false;
    let mut django_block_depth: usize = 0;

    let style_attr_re = STYLE_ATTR_RE.get_or_init(|| Regex::new(r#"^style\s*="#).unwrap());
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

        if prev_was_django_block || (is_closing_like && django_block_depth > 0 && idx > 0) {
            final_content.push_str(filler.trim());
        } else {
            final_content.push_str(filler);
        }

        let normalized = if style_attr_re.is_match(attr) {
            format_style_attribute(attr, "")
        } else {
            normalize_django(attr)
        };

        final_content.push_str(&normalized);

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
    if prev_was_django_block {
        final_content.push_str(filler.trim());
    } else {
        final_content.push_str(filler);
    }

    let normalized_final_content = whitespace_re.replace_all(&final_content, " ");
    let attrs_total_len = normalized_final_content.trim_start().len();

    let total_line_len =
        (indent_level * config.indent) + name.len() + 1 + normalized_final_content.len() + 1;

    if (attrs_total_len <= config.max_attribute_length && total_line_len <= config.max_line_length)
        || !should_wrap_attributes(name)
    {
        let mut formatted = if raw.starts_with("</") {
            format!("</{}", name)
        } else {
            format!("<{}", name)
        };

        formatted.push_str(normalized_final_content.trim_end());

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
            let _is_closing = tag_name.starts_with("end");
            let actual_tag_name = if let Some(stripped) = tag_name.strip_prefix("end") {
                stripped
            } else {
                tag_name
            };
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
            let _is_closing = tag_name.starts_with("end");
            let actual_tag_name = if let Some(stripped) = tag_name.strip_prefix("end") {
                stripped
            } else {
                tag_name
            };
            !is_block_tag(actual_tag_name, &config.custom_blocks)
        }
        Token::Text { raw, .. } => !raw.starts_with('\n') && !raw.starts_with("\r\n"),
        Token::Tag { name, .. } => is_inline_tag(name),
        Token::Comment { raw, .. } | Token::DjangoComment { raw, .. } => !raw.contains('\n'),
        Token::Doctype { .. } => false,
    }
}

fn is_block_tag(name: &str, custom_blocks: &[String]) -> bool {
    let name_lower = name.to_lowercase();
    let actual_name = if let Some(stripped) = name_lower.strip_prefix("end") {
        stripped
    } else {
        &name_lower
    };

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
    blocks.contains(&actual_name)
        || custom_blocks
            .iter()
            .any(|b| b.to_lowercase() == actual_name)
}

fn get_children_info(index: usize, tokens: &[Token]) -> (Vec<usize>, Option<usize>) {
    let mut children = Vec::new();
    let token = &tokens[index];
    match token {
        Token::Tag { name, .. } => {
            let mut j = index + 1;
            let mut depth = 1;
            let tag_name = name.to_lowercase();
            while j < tokens.len() {
                match &tokens[j] {
                    Token::Tag {
                        name: n,
                        is_closing: false,
                        is_self_closing: false,
                        ..
                    } if n.to_lowercase() == tag_name => {
                        depth += 1;
                    }
                    Token::Tag {
                        name: n,
                        is_closing: true,
                        ..
                    } if n.to_lowercase() == tag_name => {
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
            Token::Tag {
                name,
                is_closing: false,
                is_self_closing: false,
                ..
            } => {
                let start = idx;
                let mut depth = 1;
                let tag_name = name.to_lowercase();
                i += 1;
                while i < children.len() && depth > 0 {
                    let next_idx = children[i];
                    match &tokens[next_idx] {
                        Token::Tag {
                            name: n,
                            is_closing: false,
                            is_self_closing: false,
                            ..
                        } if n.to_lowercase() == tag_name => {
                            depth += 1;
                        }
                        Token::Tag {
                            name: n,
                            is_closing: true,
                            ..
                        } if n.to_lowercase() == tag_name => {
                            depth -= 1;
                        }
                        _ => {}
                    }
                    i += 1;
                }
                elements.push(start..children[i - 1] + 1);
            }
            Token::DjangoBlock { raw, .. } => {
                let tag_name = get_django_tag_name(raw).unwrap_or("");
                let is_block = is_block_tag(tag_name, &[]);
                let is_closing = tag_name.starts_with("end");

                if is_block && !is_closing {
                    let start = idx;
                    let end_tag_name = format!("end{}", tag_name);
                    let mut depth = 1;
                    i += 1;
                    while i < children.len() && depth > 0 {
                        let next_idx = children[i];
                        if let Token::DjangoBlock { raw: r, .. } = &tokens[next_idx] {
                            let name = get_django_tag_name(r).unwrap_or("");
                            if name == tag_name {
                                depth += 1;
                            } else if name == end_tag_name {
                                depth -= 1;
                            }
                        }
                        i += 1;
                    }
                    elements.push(start..children[i - 1] + 1);
                } else {
                    elements.push(idx..idx + 1);
                    i += 1;
                }
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
    let token = &tokens[range.start];
    match token {
        Token::Tag {
            name,
            raw,
            is_self_closing,
            ..
        } => {
            // Check if it's closed
            let last_token = &tokens[range.end - 1];
            if let Token::Tag {
                name: n,
                is_closing: true,
                ..
            } = last_token
            {
                if n.to_lowercase() != name.to_lowercase() {
                    return false;
                }
            } else if !is_self_closing {
                return false;
            }

            let formatted = format_tag(name, raw, *is_self_closing, 0, config);
            if formatted.contains('\n') {
                return false;
            }
        }
        Token::DjangoBlock { raw, .. } => {
            let tag_name = get_django_tag_name(raw).unwrap_or("");
            let is_block = is_block_tag(tag_name, &config.custom_blocks);
            if is_block {
                return false;
            }
        }
        _ => {}
    }

    let children_indices: Vec<usize> = (range.start + 1..range.end - 1).collect();
    let logical_elements = get_logical_elements(&children_indices, tokens);

    if logical_elements.is_empty() {
        return true;
    }

    logical_elements.iter().all(|range| {
        if range.len() == 1 {
            is_strictly_inline(&tokens[range.start], config)
        } else {
            // It's a tag pair.
            let first_token = &tokens[range.start];
            match first_token {
                Token::Tag { name: n, .. } => {
                    is_inline_tag(n) && is_tag_range_inlinable(range, tokens, config)
                }
                Token::DjangoBlock { .. } => is_tag_range_inlinable(range, tokens, config),
                _ => false,
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
