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
    /// Stack of (token_pos, incremented, tag_was_wrapped).
    /// `tag_was_wrapped` is true when the opening tag's attributes wrapped
    /// across multiple lines, meaning children and the closing tag should
    /// be on their own lines.
    parent_stack: Vec<(usize, bool, bool)>,
    last_increment_line: Option<usize>,
    last_decrement_line: Option<usize>,
    output_line_index: usize,
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
            last_increment_line: None,
            last_decrement_line: None,
            output_line_index: 0,
        }
    }

    fn push_newline(&mut self) {
        self.output.push('\n');
        self.output_line_index += 1;
        self.at_start_of_line = true;
    }

    fn push_content(&mut self, s: &str) {
        if s.contains('\n') || s.contains('\r') {
            for (idx, line) in s.split('\n').enumerate() {
                if idx > 0 {
                    self.output_line_index += 1;
                    self.at_start_of_line = true;
                }
                self.output.push_str(line);
                if idx < s.split('\n').count() - 1 {
                    self.output.push('\n');
                } else if !line.trim().is_empty() {
                    self.at_start_of_line = false;
                }
            }
        } else {
            self.output.push_str(s);
            if !s.trim().is_empty() {
                self.at_start_of_line = false;
            }
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
                    self.push_content(raw.trim());
                    self.push_newline();
                    return;
                }
            }
            self.push_content(raw);
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
            self.push_content(raw);
            self.at_start_of_line = raw.ends_with('\n');
        } else {
            self.output
                .push_str(&" ".repeat(self.indent_level * self.config.indent));
            self.push_content("<!DOCTYPE html>\n");
            self.at_start_of_line = true;
        }
    }

    fn handle_comment(&mut self, token: &Token) {
        let raw = token.raw();
        if !self.verbatim_tags.is_empty() {
            self.push_content(raw);
            self.at_start_of_line = raw.ends_with('\n');
        } else {
            let is_control = raw.contains("djlint:off") || raw.contains("djlint:on");
            if !is_control {
                if !self.at_start_of_line {
                    trim_trailing_whitespace(&mut self.output);
                    self.push_newline();
                }
                self.output
                    .push_str(&" ".repeat(self.indent_level * self.config.indent));
                self.push_content(raw.trim());
                self.push_newline();
            } else {
                self.push_content(raw.trim());
                if raw.contains("djlint:off") {
                    self.formatting_enabled = false;
                    self.at_start_of_line = false;
                } else {
                    self.push_newline();
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

                    let was_incremented = self
                        .parent_stack
                        .pop()
                        .map(|(_, inc, _)| inc)
                        .unwrap_or(false);

                    if was_incremented {
                        let already_decremented =
                            self.last_decrement_line == Some(self.output_line_index);
                        if !already_decremented {
                            self.indent_level = self.indent_level.saturating_sub(1);
                            self.last_decrement_line = Some(self.output_line_index);
                        }
                    }

                    if !self.at_start_of_line && !is_inline_tag(name) && !is_closing_verbatim {
                        trim_trailing_whitespace(&mut self.output);
                        self.push_newline();
                    }
                    if self.at_start_of_line {
                        self.output
                            .push_str(&" ".repeat(self.indent_level * self.config.indent));
                    }
                    self.push_content(&format!("</{}>", name));

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
                        self.push_newline();
                    } else {
                        self.at_start_of_line = false;
                    }
                    return;
                } else {
                    self.push_content(raw);
                    self.at_start_of_line = raw.ends_with('\n');
                    return;
                }
            }

            if *is_closing {
                let popped = self.parent_stack.pop();
                let was_incremented = popped.map(|(_, inc, _)| inc).unwrap_or(false);
                let tag_was_wrapped = popped.map(|(_, _, tw)| tw).unwrap_or(false);

                // Only decrement if the opening tag actually incremented.
                if was_incremented {
                    let already_decremented =
                        self.last_decrement_line == Some(self.output_line_index);
                    if !already_decremented {
                        self.indent_level = self.indent_level.saturating_sub(1);
                        self.last_decrement_line = Some(self.output_line_index);
                    }
                }

                // Push a newline before the closing tag when:
                // - not at start of line AND
                // - the tag is a block-level tag, OR
                // - the tag is inline but had wrapped attributes (meaning
                //   children are on separate lines from the opening tag)
                if !self.at_start_of_line && (!is_inline_tag(name) || tag_was_wrapped) {
                    trim_trailing_whitespace(&mut self.output);
                    self.push_newline();
                }
                if self.at_start_of_line {
                    self.output
                        .push_str(&" ".repeat(self.indent_level * self.config.indent));
                }
                self.push_content(&format!("</{}>", name));

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
                    self.push_newline();
                } else {
                    self.at_start_of_line = false;
                }
            } else {
                let (children, closing_idx) = get_children_info(self.pos, &self.tokens);

                let is_potentially_verbatim =
                    matches!(name_lower.as_str(), "style" | "script") && !is_self_closing;

                if !self.at_start_of_line && (is_html_block_tag(name) || is_potentially_verbatim) {
                    trim_trailing_whitespace(&mut self.output);
                    self.push_newline();
                }

                let was_at_start_of_line = self.at_start_of_line;

                if self.at_start_of_line {
                    self.output
                        .push_str(&" ".repeat(self.indent_level * self.config.indent));
                }

                let formatted_tag =
                    format_tag(name, raw, *is_self_closing, self.indent_level, self.config);
                self.push_content(&formatted_tag);

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
                                is_strictly_inline(&self.tokens[range.start], self.config, false)
                            } else {
                                // It's a tag pair. Inline if it's an inline tag and its content is inlinable.
                                if let Token::Tag { name: n, .. } = &self.tokens[range.start] {
                                    is_inline_tag(n)
                                        && is_tag_range_inlinable(
                                            range,
                                            &self.tokens,
                                            self.config,
                                            false,
                                        )
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
                            // Check all tokens in the range for newlines,
                            // not just standalone text tokens. This catches
                            // newlines inside child tag content (e.g.,
                            // <b>text\n  text</b>).
                            (range.start..range.end).any(|idx| {
                                if let Token::Text { raw, .. } = &self.tokens[idx] {
                                    raw.contains('\n')
                                } else {
                                    false
                                }
                            })
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
                                // No child tags. Block collapse only when
                                // newlines separate multiple content pieces
                                // (not just indentation around one piece).
                                !has_newline_text || non_whitespace_elements.len() <= 1
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

                                    if true {
                                        eprintln!(
                                            "DEBUG: {} tag projected_len: {}, max: {}",
                                            name, projected_len, self.config.max_line_length
                                        );
                                    }

                                    if (projected_len <= self.config.max_line_length
                                        || is_potentially_verbatim)
                                        && (logical_elements.is_empty()
                                            && j == self.pos + 1
                                            && self.tokens[j].line() == token.ends_on_line()
                                            || !logical_elements.is_empty())
                                    {
                                        self.push_content(collapsed_content);
                                        self.push_content(&format!("</{}>", name));
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
                                            self.push_newline();
                                        } else {
                                            self.at_start_of_line = false;
                                        }
                                        self.pos = j;
                                        did_collapse = true;
                                    }
                                }
                            }

                            /*
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
                            self.push_newline();
                                        self.output
                                            .push_str(&" ".repeat(child_indent * self.config.indent));
                                        self.push_content(collapsed_content);
                                        trim_trailing_whitespace(&mut self.output);
                            self.push_newline();
                                        self.output
                                            .push_str(&" ".repeat(self.indent_level * self.config.indent));
                                        self.push_content(&format!("</{}>", name));
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
                                    self.push_newline();
                                        } else {
                                            self.at_start_of_line = false;
                                        }
                                        self.pos = j;
                                        did_collapse = true;
                                    }
                                    */
                        }
                    }
                }

                if !did_collapse {
                    let mut is_verbatim = false;
                    if is_potentially_verbatim {
                        is_verbatim = true;
                        self.verbatim_tags.push(name_lower.clone());
                    }

                    let tag_was_wrapped = formatted_tag.contains('\n');

                    if !is_verbatim {
                        let mut should_newline = true;
                        if !is_structural && !tag_was_wrapped && self.pos + 1 < self.tokens.len() {
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
                            self.push_newline();
                        } else {
                            self.at_start_of_line = false;
                        }
                    } else {
                        self.at_start_of_line =
                            formatted_tag.ends_with('\n') || formatted_tag.ends_with("\r\n");
                    }

                    // Reset last_increment_line when we pushed a newline.
                    // The increment below will be on the NEW line (where
                    // children will be written), and we don't want it to
                    // block the first child from also incrementing.
                    let pushed_newline = self.at_start_of_line;

                    let mut incremented = false;

                    // Inline tags that appear mid-line (not at the start
                    // of a line) should not increment indent. This matches
                    // djlint where inline tags mid-line don't affect
                    // indentation (e.g., "by <span>" keeps <span>'s
                    // children at the same level).
                    let inline_mid_line = is_inline_tag(name) && !was_at_start_of_line;

                    if !is_self_closing
                        && should_indent_children(name)
                        && !is_verbatim
                        && !inline_mid_line
                    {
                        // If we just pushed a newline, the children will be
                        // on a new line. Clear the dedup so the first child
                        // can increment independently.
                        let already_incremented = if pushed_newline {
                            false
                        } else {
                            self.last_increment_line == Some(self.output_line_index)
                        };
                        if !already_incremented {
                            self.indent_level += 1;
                            self.last_increment_line = Some(self.output_line_index);
                            incremented = true;
                        }
                    }

                    if !is_self_closing {
                        self.parent_stack
                            .push((self.pos, incremented, tag_was_wrapped));
                    }
                }
            }
        }
    }

    fn handle_text(&mut self, token: &Token) {
        let raw = token.raw();
        if !self.verbatim_tags.is_empty() {
            self.push_content(raw);
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
                            self.push_newline();
                        }
                        continue;
                    }
                    blank_lines = 0;

                    if self.at_start_of_line {
                        self.output
                            .push_str(&" ".repeat(self.indent_level * self.config.indent));
                        self.push_content(line.trim_start());
                    } else if idx == 0 {
                        if raw.starts_with('\n') || raw.starts_with("\r\n") {
                            trim_trailing_whitespace(&mut self.output);
                            self.push_newline();
                            self.output
                                .push_str(&" ".repeat(self.indent_level * self.config.indent));
                            self.push_content(line.trim_start());
                        } else {
                            // Continuing inline. We want to preserve original leading spaces
                            let leading_spaces = raw.chars().take_while(|&c| c == ' ').count();
                            if leading_spaces > 0 {
                                self.output.push_str(&" ".repeat(leading_spaces));
                            }
                            self.push_content(line);
                        }
                    } else {
                        trim_trailing_whitespace(&mut self.output);
                        self.push_newline();
                        self.output
                            .push_str(&" ".repeat(self.indent_level * self.config.indent));
                        self.push_content(line.trim_start());
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
                            self.push_newline();
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
                        self.push_newline();
                    }
                }
            } else if !raw.is_empty() {
                if raw.contains('\n') {
                    if !self.at_start_of_line {
                        trim_trailing_whitespace(&mut self.output);
                        self.push_newline();
                    }
                    self.output_line_index += raw.chars().filter(|&c| c == '\n').count();
                    self.at_start_of_line = true;
                } else if !self.at_start_of_line {
                    self.push_content(raw);
                }
            }
        }
    }

    fn handle_django_var(&mut self, token: &Token) {
        let raw = token.raw();
        if !self.verbatim_tags.is_empty() {
            self.push_content(raw);
            self.at_start_of_line = raw.ends_with('\n');
        } else {
            if self.at_start_of_line {
                self.output
                    .push_str(&" ".repeat(self.indent_level * self.config.indent));
            }
            self.push_content(&normalize_django(raw));

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
                self.push_newline();
            } else {
                self.at_start_of_line = false;
            }
        }
    }

    fn handle_django_block(&mut self, token: &Token) {
        let raw = token.raw();
        if !self.verbatim_tags.is_empty() {
            let is_script_or_style = self
                .verbatim_tags
                .last()
                .map(|t| t == "script" || t == "style")
                .unwrap_or(false);
            if is_script_or_style {
                let tag_name = get_django_tag_name(raw).unwrap_or("");
                let is_block = is_block_tag(tag_name, &self.config.custom_blocks);
                if is_block && !tag_name.starts_with("end") {
                    let (children, closing_idx) = get_children_info(self.pos, &self.tokens);
                    if let Some(j) = closing_idx {
                        let logical_elements = get_logical_elements(&children, &self.tokens);
                        let all_inline_ish = logical_elements.iter().all(|range| {
                            if range.len() == 1 {
                                is_strictly_inline(&self.tokens[range.start], self.config, true)
                            } else {
                                false
                            }
                        });
                        if all_inline_ish && !logical_elements.is_empty() {
                            let content = format_range_inlined_joined(
                                &logical_elements,
                                &self.tokens,
                                0,
                                self.config,
                            );
                            if !content.contains('\n') {
                                let normalized_start = normalize_django(raw);
                                let normalized_end = normalize_django(self.tokens[j].raw());
                                self.push_content(&normalized_start);
                                self.push_content(content.trim());
                                self.push_content(&normalized_end);
                                self.at_start_of_line = false;
                                self.pos = j;
                                return;
                            }
                        }
                    }
                }
            }
            self.push_content(raw);
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
                self.parent_stack.pop();

                let already_decremented = self.last_decrement_line == Some(self.output_line_index);
                if !already_decremented {
                    self.indent_level = self.indent_level.saturating_sub(1);
                    self.last_decrement_line = Some(self.output_line_index);
                }
            }

            // Check if we can inline
            let mut did_collapse = false;

            if !self.at_start_of_line && (is_block || is_reindent) {
                trim_trailing_whitespace(&mut self.output);
                self.push_newline();
            }

            if !is_closing && is_block {
                let (children, closing_idx) = get_children_info(self.pos, &self.tokens);

                if let Some(j) = closing_idx {
                    let logical_elements = get_logical_elements(&children, &self.tokens);

                    let all_inline_ish = logical_elements.iter().all(|range| {
                        if range.len() == 1 {
                            is_strictly_inline(&self.tokens[range.start], self.config, true)
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
                                                true,
                                            )
                                    } else {
                                        is_inline_tag(n)
                                            && is_tag_range_inlinable(
                                                range,
                                                &self.tokens,
                                                self.config,
                                                true,
                                            )
                                    }
                                }
                                Token::DjangoBlock { .. } => {
                                    is_tag_range_inlinable(range, &self.tokens, self.config, true)
                                }
                                _ => false,
                            }
                        }
                    });

                    if all_inline_ish {
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

                        let normalized_start = normalize_django(raw);
                        let normalized_end = normalize_django(self.tokens[j].raw());
                        let content = format_range_inlined_joined(
                            &logical_elements,
                            &self.tokens,
                            self.indent_level + 1,
                            self.config,
                        );
                        let collapsed_content = content.trim();

                        // djlint's condense step does not count indentation
                        // in the line length check — it only uses 1 space
                        // before the opening tag. We match that behavior so
                        // deeply-nested single-child blocks still collapse.
                        let condensed_len = 1
                            + normalized_start.len()
                            + collapsed_content.len()
                            + normalized_end.len();

                        let all_strictly_inline = logical_elements.iter().all(|range| {
                            if range.len() == 1 {
                                is_strictly_inline(&self.tokens[range.start], self.config, true)
                            } else {
                                let first_token = &self.tokens[range.start];
                                if let Token::Tag { name: n, .. } = first_token {
                                    is_inline_tag(n)
                                } else {
                                    false
                                }
                            }
                        });

                        if condensed_len < self.config.max_line_length
                            && (all_strictly_inline || non_whitespace_elements.len() <= 1)
                        {
                            if self.at_start_of_line {
                                self.output
                                    .push_str(&" ".repeat(self.indent_level * self.config.indent));
                            }
                            self.push_content(&normalized_start);
                            self.push_content(collapsed_content);
                            self.push_content(&normalized_end);
                            trim_trailing_whitespace(&mut self.output);
                            self.push_newline();

                            // Track unclosed HTML tags in the collapsed
                            // content. djlint's regex-based indent sees
                            // opening tags and increments even inside
                            // condensed blocks, so unclosed tags "leak"
                            // indent to subsequent lines.
                            for &child_idx in &children {
                                match &self.tokens[child_idx] {
                                    Token::Tag {
                                        name: n,
                                        is_closing: false,
                                        is_self_closing: false,
                                        ..
                                    } if is_inline_tag(n) || is_html_block_tag(n) => {
                                        self.indent_level += 1;
                                    }
                                    Token::Tag {
                                        name: n,
                                        is_closing: true,
                                        ..
                                    } if is_inline_tag(n) || is_html_block_tag(n) => {
                                        self.indent_level = self.indent_level.saturating_sub(1);
                                    }
                                    _ => {}
                                }
                            }

                            self.pos = j;
                            did_collapse = true;
                        }
                    }
                }
            }

            if !did_collapse {
                if self.at_start_of_line {
                    self.output
                        .push_str(&" ".repeat(self.indent_level * self.config.indent));
                }
                self.push_content(&normalize_django(raw));

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
                    self.push_newline();
                } else {
                    self.at_start_of_line = false;
                }

                if (!is_closing && is_block) || is_reindent {
                    let (_, closing_idx) = get_children_info(self.pos, &self.tokens);
                    if closing_idx.is_some() || is_reindent {
                        let already_incremented =
                            self.last_increment_line == Some(self.output_line_index);
                        let mut incremented = false;
                        if !already_incremented {
                            self.indent_level += 1;
                            self.last_increment_line = Some(self.output_line_index);
                            incremented = true;
                        }
                        self.parent_stack.push((self.pos, incremented, false));
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
        .map(|m| {
            let normalized = normalize_django(m.as_str());
            // Collapse internal whitespace (e.g., multi-line attribute values)
            whitespace_re.replace_all(&normalized, " ").to_string()
        })
        .collect();

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

        if prev_was_django_block || (is_closing_like && django_block_depth > 0 && idx > 0) {
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
    if prev_was_django_block {
        raw_final_content.push_str(filler.trim());
    } else {
        raw_final_content.push_str(filler);
    }

    // Collapse whitespace (e.g., multi-line attributes) to single spaces.
    // This string preserves style attribute values as-is (including trailing
    // semicolons), matching djlint which only reformats style when wrapping.
    let raw_attrs_collapsed = whitespace_re.replace_all(&raw_final_content, " ");
    let raw_attrs_len = raw_attrs_collapsed.trim().len();

    let total_line_len =
        (indent_level * config.indent) + name.len() + 1 + raw_attrs_collapsed.len() + 1;

    if (raw_attrs_len < config.max_attribute_length && total_line_len <= config.max_line_length)
        || !should_wrap_attributes(name)
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

fn is_strictly_inline(token: &Token, config: &Config, is_parent_django: bool) -> bool {
    match token {
        Token::DjangoVar { .. } => true,
        Token::DjangoBlock { raw, .. } => {
            if is_parent_django {
                return false;
            }
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
        Token::Tag { name, raw, .. } => {
            if is_parent_django {
                is_inline_tag(name) && !raw.contains("{%")
            } else {
                is_inline_tag(name) && !raw.contains("{%") && !raw.contains("{{")
            }
        }
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
        Token::Tag { name, .. } => {
            let res = is_inline_tag(name);
            if *name == "b" {
                // eprintln!("DEBUG: is_inline_ish for b: {}", res);
            }
            res
        }
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
    is_parent_django: bool,
) -> bool {
    let token = &tokens[range.start];
    match token {
        Token::Tag {
            name,
            raw,
            is_self_closing,
            ..
        } => {
            if raw.contains("{%") || (!is_parent_django && raw.contains("{{")) {
                return false;
            }
            // Check if it's closed. Unclosed inline tags (like <span>
            // without </span>) are still inlinable — djlint condenses
            // them as part of the surrounding content.
            let last_token = &tokens[range.end - 1];
            let is_properly_closed = if let Token::Tag {
                name: n,
                is_closing: true,
                ..
            } = last_token
            {
                n.to_lowercase() == name.to_lowercase()
            } else {
                *is_self_closing
            };

            if !is_properly_closed && !is_inline_tag(name) {
                return false;
            }

            let formatted = format_tag(name, raw, *is_self_closing, 0, config);
            if formatted.contains('\n') {
                return false;
            }
        }
        Token::DjangoBlock { raw, .. } => {
            if is_parent_django {
                return false;
            }
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
            is_strictly_inline(&tokens[range.start], config, is_parent_django)
        } else {
            // It's a tag pair.
            let first_token = &tokens[range.start];
            match first_token {
                Token::Tag { name: n, .. } => {
                    is_inline_tag(n)
                        && is_tag_range_inlinable(range, tokens, config, is_parent_django)
                }
                Token::DjangoBlock { .. } => {
                    is_tag_range_inlinable(range, tokens, config, is_parent_django)
                }
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
