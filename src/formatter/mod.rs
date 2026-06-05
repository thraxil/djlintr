pub mod tokenizer;

use crate::config::Config;
use crate::tags::{
    is_condensable_tag, is_html_block_tag, is_inline_tag, is_structural_tag,
    should_indent_children, should_wrap_attributes,
};
use regex::Regex;
use std::sync::OnceLock;
use tokenizer::{Token, Tokenizer};

struct Formatter<'a> {
    config: &'a Config,
    output: String,
    indent_level: usize,
    tokens: Vec<Token<'a>>,
    pos: usize,
    formatting_enabled: bool,
    verbatim_tags: Vec<String>,
    at_start_of_line: bool,
    /// Stack of (token_pos, incremented, tag_was_wrapped, was_inline_mid_line).
    parent_stack: Vec<(usize, bool, bool, bool)>,
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

    /// Push indentation spaces for the current indent level.
    fn push_indent(&mut self) {
        self.output
            .push_str(&" ".repeat(self.indent_level * self.config.indent));
    }

    /// Push indentation, optionally at one level less than the current
    /// indent.  Used when a text line will be followed inline by a
    /// closing inline tag (djlint dedents such lines).
    fn push_indent_maybe_dedented(&mut self, dedent: bool) {
        if dedent {
            let level = self.indent_level.saturating_sub(1);
            self.output
                .push_str(&" ".repeat(level * self.config.indent));
        } else {
            self.push_indent();
        }
    }

    /// Trim trailing whitespace from the output and push a newline.
    fn trim_and_newline(&mut self) {
        trim_trailing_whitespace(&mut self.output);
        self.push_newline();
    }

    /// Check whether the next token continues on the same line (inline),
    /// meaning we should NOT emit a newline after the current token.
    /// Returns `true` if we should continue inline (i.e., skip the newline).
    fn should_continue_inline(&self, token: &Token, guard: bool) -> bool {
        if !guard || self.pos + 1 >= self.tokens.len() {
            return false;
        }
        let next_token = &self.tokens[self.pos + 1];

        // If the text ends with a newline (ignoring trailing spaces/tabs),
        // then the next token is on a new source line and not inline.
        let ends_with_newline = match token {
            Token::Text { raw, .. } => {
                let trimmed = raw.trim_end_matches([' ', '\t']);
                trimmed.ends_with('\n') || trimmed.ends_with("\r\n")
            }
            _ => token.raw().ends_with('\n') || token.raw().ends_with("\r\n"),
        };

        if next_token.line() == token.ends_on_line()
            && !ends_with_newline
            && is_inline_ish(next_token, self.config)
        {
            return true;
        }
        if let Token::Text { raw: r, .. } = next_token {
            if r.starts_with('\n') || r.starts_with("\r\n") {
                return true;
            }
        }
        false
    }

    /// Emit a newline unless the next token continues inline.
    /// `guard` controls whether inline continuation is even possible
    /// (e.g., `is_inline_tag(name)`). When false, always emits a newline.
    fn emit_newline_or_continue(&mut self, token: &Token, guard: bool) {
        if self.should_continue_inline(token, guard) {
            self.at_start_of_line = false;
        } else {
            self.trim_and_newline();
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
                    self.at_start_of_line = self.output.ends_with('\n')
                        || self.output.ends_with("\r\n")
                        || self.output.is_empty();
                    if self.at_start_of_line {
                        self.push_indent();
                    }
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
            self.push_indent();
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
                    self.trim_and_newline();
                }
                self.push_indent();
                self.push_content(raw.trim());
                self.push_newline();
            } else {
                if !self.at_start_of_line {
                    self.trim_and_newline();
                }
                self.push_indent();
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
                        .map(|(_, inc, _, _)| inc)
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
                        self.trim_and_newline();
                    }
                    if self.at_start_of_line {
                        // For closing verbatim tags (</script>, </style>),
                        // the verbatim content may have left whitespace
                        // (tabs/spaces) on the current line. Trim it so
                        // our space-based indent replaces it cleanly.
                        if is_closing_verbatim {
                            trim_trailing_whitespace(&mut self.output);
                        }
                        self.push_indent();
                    }
                    self.push_content(&format!("</{}>", name));
                    self.emit_newline_or_continue(
                        token,
                        is_inline_tag(name) || is_closing_verbatim,
                    );
                    return;
                } else {
                    self.push_content(raw);
                    self.at_start_of_line = raw.ends_with('\n');
                    return;
                }
            }

            if *is_closing {
                let popped = self.parent_stack.pop();
                let popped_pos = popped.map(|(pos, _, _, _)| pos);
                let was_incremented = popped.map(|(_, inc, _, _)| inc).unwrap_or(false);
                let was_inline_mid_line = popped.map(|(_, _, _, ml)| ml).unwrap_or(false);

                // djlint only unindents for closing tags at the start or
                // end of a (stripped) line. A closing inline tag that
                // appears mid-line with inline content following it on the
                // same source line should NOT decrement, because the
                // continuation text keeps the visual indent level alive.
                let closing_midline_with_trailing = is_inline_tag(name)
                    && !self.at_start_of_line
                    && self.pos + 1 < self.tokens.len()
                    && {
                        let next = &self.tokens[self.pos + 1];
                        next.line() == token.ends_on_line() && is_inline_ish(next, self.config)
                    };

                // When an inline tag opens AND closes on the same source
                // line (e.g. `<b class="long">Short</b> <i>…</i>` all on
                // one line), the tag's indent increment is self-contained
                // and must be reversed at the close — even when there is
                // trailing inline content on the same line — otherwise the
                // parent's closing tags end up over-indented.
                //
                // When open and close span different source lines, keep
                // the existing closing_midline_with_trailing guard: djlint
                // preserves the incremented level for sibling content that
                // follows the closing tag on the same line.
                let open_close_same_line = popped_pos
                    .map(|pos| self.tokens[pos].line() == token.line())
                    .unwrap_or(false);

                let should_decrement = if open_close_same_line && was_incremented {
                    true
                } else {
                    (was_incremented
                        || (was_inline_mid_line
                            && should_indent_children(name)
                            && self.at_start_of_line))
                        && !closing_midline_with_trailing
                };
                if should_decrement {
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
                // Note: djlint preserves inline children on the same line if they
                // didn't force a newline themselves, even if the opening tag wrapped.
                // However, if the children DID force a newline, we should put the
                // closing tag on its own line.
                // But wait, if children forced a newline, at_start_of_line is true!
                // So if we are NOT at the start of the line, the last child was inline.
                // In Python djlint, if the last child was inline, the closing tag stays inline.
                let force_newline = !is_inline_tag(name);

                // If it's an inline tag whose attributes wrapped, and its last child
                // was a block element (which is invalid HTML but possible), or it had
                // block content, it would already be on a new line. But if the last child
                // was inline, we should keep it inline to match Python djlint.
                if !self.at_start_of_line && force_newline {
                    self.trim_and_newline();
                }
                if self.at_start_of_line {
                    self.push_indent();
                }
                self.push_content(&format!("</{}>", name));
                self.emit_newline_or_continue(token, is_inline_tag(name));
            } else {
                let (children, closing_idx) = get_children_info(self.pos, &self.tokens);

                // Detect a "mismatched void": an opening tag with no
                // matching close that is immediately followed by a closing
                // tag of a *different* name (e.g. `<span ...></i>`).
                // djlint's line-based indenter treats this as
                // self-balancing (the unindent branch wins and the open
                // tag never contributes to indent).  We mirror this by
                // treating the pair as a void element for indent purposes:
                // emit both tags inline, skip indent increment, and skip
                // the parent_stack push so the mismatched close won't
                // incorrectly pop a real parent.
                let mismatched_void_close = if closing_idx.is_none() && !is_self_closing {
                    // Check if the immediately next token is a closing tag
                    // with a different name
                    let next_pos = self.pos + 1;
                    if next_pos < self.tokens.len() {
                        if let Token::Tag {
                            name: close_name,
                            is_closing: true,
                            ..
                        } = &self.tokens[next_pos]
                        {
                            if close_name.to_lowercase() != name_lower {
                                Some(next_pos)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };

                // An "unclosed orphan" is an opening tag that has no matching
                // close anywhere in the document AND whose next non-whitespace
                // sibling is a closing tag of a different (parent) element.
                // Example: `<circle>` inside `<svg>...</svg>` with no
                // `</circle>`.  djlint's line-based indenter never increments
                // for such tags because it never sees a matching unindent line.
                // We mirror this by skipping the indent-stack push so the
                // parent's closing tag correctly pops its own entry.
                let is_unclosed_orphan = mismatched_void_close.is_none()
                    && closing_idx.is_none()
                    && !is_self_closing
                    && {
                        let mut np = self.pos + 1;
                        while np < self.tokens.len() {
                            match &self.tokens[np] {
                                Token::Text { raw, .. } if raw.trim().is_empty() => np += 1,
                                _ => break,
                            }
                        }
                        matches!(
                            self.tokens.get(np),
                            Some(Token::Tag { name: cn, is_closing: true, .. })
                                if cn.to_lowercase() != name_lower
                        )
                    };

                let is_potentially_verbatim =
                    matches!(name_lower.as_str(), "style" | "script" | "pre" | "textarea")
                        && !is_self_closing;
                let is_ignored_block =
                    matches!(name_lower.as_str(), "pre" | "textarea") && !is_self_closing;

                if !self.at_start_of_line && (is_html_block_tag(name) || is_potentially_verbatim) {
                    self.trim_and_newline();
                }

                // When a mismatched void is detected (e.g. `<span></i>`),
                // djlint's line-based indenter sees the `</i>` at the end
                // of the line and runs the unindent branch BEFORE placing
                // the line.  Mirror this by decrementing before output.
                if mismatched_void_close.is_some() && self.at_start_of_line {
                    self.indent_level = self.indent_level.saturating_sub(1);
                }

                let was_at_start_of_line = self.at_start_of_line;

                if self.at_start_of_line {
                    self.push_indent();
                }

                // Match djlint's `leading_space + len("<tagname ")` formula:
                // count the trailing spaces currently in the output (which
                // is the whitespace immediately before the opening `<`).
                let spaces_before_tag = self.output.chars().rev().take_while(|&c| c == ' ').count();
                let formatted_tag = format_tag(
                    name,
                    raw,
                    *is_self_closing,
                    self.indent_level,
                    spaces_before_tag,
                    self.config,
                    is_ignored_block,
                );
                self.push_content(&formatted_tag);

                self.at_start_of_line =
                    formatted_tag.ends_with('\n') || formatted_tag.ends_with("\r\n");

                // Try to collapse children inline (e.g., <p>text</p>).
                let did_collapse = !is_self_closing
                    && !is_ignored_block
                    && self.try_collapse_html_tag(
                        token,
                        name,
                        &children,
                        closing_idx,
                        &formatted_tag,
                        is_potentially_verbatim,
                    );

                if let (false, Some(close_pos)) = (did_collapse, mismatched_void_close) {
                    // Mismatched void: emit the closing tag inline, no
                    // indent change, no parent_stack push.  Advance pos
                    // past the mismatched closer so it is not processed
                    // again.
                    if let Token::Tag {
                        name: close_name, ..
                    } = &self.tokens[close_pos]
                    {
                        self.push_content(&format!("</{}>", close_name));
                    }
                    self.pos = close_pos;
                    // Decide whether to push a newline after the pair.
                    // Note: mismatched void uses a simplified inline check
                    // (no ends_with('\n') guard since token is synthetic).
                    let continue_inline = self.pos + 1 < self.tokens.len() && {
                        let next_token = &self.tokens[self.pos + 1];
                        next_token.line() == self.tokens[close_pos].ends_on_line()
                            && is_inline_ish(next_token, self.config)
                    };
                    if continue_inline {
                        self.at_start_of_line = false;
                    } else {
                        self.trim_and_newline();
                    }
                    // The mismatched close acts as an unindent in djlint.
                    // Pop the most recent parent so the stack stays
                    // balanced (the opening tag we just pushed never had
                    // a matching close, and this mismatched close consumed
                    // it). Don't decrement though — we already avoided
                    // incrementing for this tag.
                    // Note: we did NOT push to parent_stack for this tag,
                    // so no pop is needed.
                } else if !did_collapse {
                    let mut is_verbatim = false;
                    if is_potentially_verbatim {
                        is_verbatim = true;
                        self.verbatim_tags.push(name_lower.clone());
                    }

                    let tag_was_wrapped = formatted_tag.contains('\n');

                    if !is_verbatim {
                        // Allow inline continuation only for non-structural,
                        // non-wrapped tags that are inline or non-block.
                        // However, inline tags like <a> should NOT be forced
                        // to wrap their content just because their attributes wrapped.
                        let inline_guard = !is_structural_tag(name)
                            && (!tag_was_wrapped || is_inline_tag(name))
                            && (is_inline_tag(name) || !is_html_block_tag(name));
                        self.emit_newline_or_continue(token, inline_guard);
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
                        && !is_unclosed_orphan
                    {
                        // Skip dedup when:
                        // - we just pushed a newline (children on a new line)
                        // - tag was at start of line (tag is a child that
                        //   should increment independently of its parent)
                        let already_incremented = if pushed_newline || was_at_start_of_line {
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

                    if !is_self_closing && !is_unclosed_orphan {
                        self.parent_stack.push((
                            self.pos,
                            incremented,
                            tag_was_wrapped,
                            inline_mid_line,
                        ));
                    }
                }
            }
        }
    }

    /// Try to collapse an HTML tag's children inline onto a single line.
    /// Returns `true` if the tag was collapsed (children inlined and closing
    /// tag emitted). The caller should skip normal indent/parent-stack
    /// handling when this returns `true`.
    fn try_collapse_html_tag(
        &mut self,
        token: &Token,
        name: &str,
        children: &[usize],
        closing_idx: Option<usize>,
        formatted_tag: &str,
        is_potentially_verbatim: bool,
    ) -> bool {
        let j = match closing_idx {
            Some(j) => j,
            None => return false,
        };

        let logical_elements = get_logical_elements(children, &self.tokens);

        let all_inline_ish = logical_elements.iter().all(|range| {
            if range.len() == 1 {
                is_strictly_inline(&self.tokens[range.start], self.config, false)
            } else {
                if let Token::Tag { name: n, .. } = &self.tokens[range.start] {
                    is_inline_tag(n)
                        && is_tag_range_inlinable(range, &self.tokens, self.config, false)
                } else {
                    false
                }
            }
        });

        if !all_inline_ish {
            return false;
        }

        let has_any_tag = logical_elements.iter().any(|range| {
            if range.len() > 1 {
                true
            } else {
                matches!(
                    &self.tokens[range.start],
                    Token::Tag { .. } | Token::Comment { .. } | Token::DjangoComment { .. }
                )
            }
        });

        let has_newline_text = logical_elements.iter().any(|range| {
            (range.start..range.end).any(|idx| {
                if let Token::Text { raw, .. } = &self.tokens[idx] {
                    raw.contains('\n')
                } else {
                    false
                }
            })
        });

        // Like has_newline_text, but only considers non-whitespace text.
        // Whitespace-only text nodes that contain newlines (e.g. "\n    "
        // between the opening tag and the first content token) don't count
        // as "real" multi-line content. djlint uses this looser check when
        // deciding whether to collapse block-level elements.
        let has_content_newline = logical_elements.iter().any(|range| {
            (range.start..range.end).any(|idx| {
                if let Token::Text { raw, .. } = &self.tokens[idx] {
                    raw.trim().contains('\n')
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

        let is_block_parent = is_html_block_tag(name);
        let is_structural = is_structural_tag(name);

        let can_collapse = if is_structural {
            false
        } else if is_block_parent {
            if non_whitespace_elements.is_empty() && is_condensable_tag(name) {
                true
            } else if non_whitespace_elements.is_empty() {
                false
            } else {
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
                ((same_line && !has_newline_text)
                    || non_whitespace_elements.len() == 1
                    || !has_content_newline)
                    && !has_any_tag
            }
        } else if has_any_tag {
            !has_newline_text
        } else {
            !has_newline_text || non_whitespace_elements.len() <= 1
        };

        if !can_collapse {
            return false;
        }

        let is_wrapped = formatted_tag.contains('\n');
        if is_wrapped && has_any_tag {
            return false;
        }

        let content = format_range_inlined_joined(
            &logical_elements,
            &self.tokens,
            self.indent_level,
            self.config,
        );
        let collapsed_content = content.trim();
        let tag_last_line_len = formatted_tag.split('\n').next_back().unwrap_or("").len();
        let current_line_len = if formatted_tag.contains('\n') {
            tag_last_line_len
        } else {
            (self.indent_level * self.config.indent) + tag_last_line_len
        };
        let projected_len = current_line_len + collapsed_content.len() + name.len() + 3;

        // djlint calculates combined length for condensation using regex:
        // len(last_line_of_open_tag + content + close_tag)
        // Which effectively ignores the indentation of the open tag
        // if the open tag is a single line, and only considers the last line
        // if it's multiline.
        let djlint_condensed_len = tag_last_line_len + collapsed_content.len() + name.len() + 3;

        // djlint ignores the outer indentation when deciding whether to
        // collapse: it checks condensed length (tag last line + content +
        // close tag) rather than the full projected line length.  Apply
        // the same skip for any element whose content has no child HTML
        // tags (only text/template vars/whitespace).
        let skip_line_length_check =
            !has_any_tag && djlint_condensed_len < self.config.max_line_length;

        if (projected_len < self.config.max_line_length
            || is_potentially_verbatim
            || skip_line_length_check)
            && (logical_elements.is_empty()
                && j == self.pos + 1
                && self.tokens[j].line() == token.ends_on_line()
                || !logical_elements.is_empty())
        {
            self.push_content(collapsed_content);
            self.push_content(&format!("</{}>", name));

            // Track indent leaked by Django block openers among the collapsed
            // children.  djlint's two-phase expand-then-condense approach
            // increments indent for every custom block opener it encounters
            // during expansion; that increment persists even when the content
            // is later condensed onto a single line.
            let indent_delta: i32 = {
                let tokens = &self.tokens;
                let custom_blocks = &self.config.custom_blocks;
                children
                    .iter()
                    .map(|&idx| {
                        if let Token::DjangoBlock { raw, .. } = &tokens[idx] {
                            let tag_name = get_django_tag_name(raw).unwrap_or("");
                            let actual = strip_end_prefix(tag_name);
                            if is_block_tag(actual, custom_blocks) {
                                if tag_name.starts_with("end") {
                                    -1i32
                                } else {
                                    1i32
                                }
                            } else {
                                0i32
                            }
                        } else {
                            0i32
                        }
                    })
                    .sum()
            };
            if indent_delta > 0 {
                self.indent_level += indent_delta as usize;
            } else if indent_delta < 0 {
                self.indent_level = self.indent_level.saturating_sub((-indent_delta) as usize);
            }

            self.pos = j;
            self.emit_newline_or_continue(token, is_inline_tag(name));
            true
        } else {
            false
        }
    }

    fn handle_text(&mut self, token: &Token) {
        let raw = token.raw();
        if !self.verbatim_tags.is_empty() {
            self.push_content(raw);
            // Don't override at_start_of_line — push_content already
            // tracks this correctly based on the content's newlines.
        } else {
            let trimmed = raw.trim();
            if !trimmed.is_empty() {
                let lines: Vec<&str> = trimmed.lines().collect();
                let mut blank_lines = 0;

                // djlint's line-based indenter applies a dedent when a
                // line ends with a closing inline tag (e.g. `</span>`).
                // Detect whether the last text line will be followed
                // inline by such a closing tag and, if so, use the
                // parent's indent level for that line.  Only apply
                // when the parent tag actually incremented indent
                // (i.e. it was at the start of a line, not mid-line).
                let last_line_closes_inline = self.should_continue_inline(token, true)
                    && self.pos + 1 < self.tokens.len()
                    && matches!(
                        &self.tokens[self.pos + 1],
                        Token::Tag {
                            name: n,
                            is_closing: true,
                            ..
                        } if is_inline_tag(n)
                    )
                    && self
                        .parent_stack
                        .last()
                        .is_some_and(|&(_, incremented, tw, _)| incremented && !tw);

                for (idx, line) in lines.iter().enumerate() {
                    let is_last_line = idx == lines.len() - 1;

                    if line.trim().is_empty() {
                        blank_lines += 1;
                        if blank_lines <= self.config.max_blank_lines {
                            self.trim_and_newline();
                        }
                        continue;
                    }
                    blank_lines = 0;

                    // Use the parent's indent when this is the last
                    // text line and will continue inline into a closing
                    // inline tag.
                    let use_dedent = is_last_line && last_line_closes_inline;

                    if self.at_start_of_line {
                        self.push_indent_maybe_dedented(use_dedent);
                        self.push_content(line.trim_start());
                    } else if idx == 0 {
                        if raw.starts_with('\n') || raw.starts_with("\r\n") {
                            self.trim_and_newline();
                            self.push_indent_maybe_dedented(use_dedent);
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
                        self.trim_and_newline();
                        self.push_indent_maybe_dedented(use_dedent);
                        self.push_content(line.trim_start());
                    }

                    if is_last_line {
                        if self.should_continue_inline(token, true) {
                            // Preserve original trailing space if any
                            let trailing_spaces =
                                raw.chars().rev().take_while(|&c| c == ' ').count();
                            if trailing_spaces > 0 {
                                self.output.push_str(&" ".repeat(trailing_spaces));
                            }
                            self.at_start_of_line = false;
                        } else {
                            self.trim_and_newline();
                        }
                    } else {
                        self.trim_and_newline();
                    }
                }
            } else if !raw.is_empty() {
                if raw.contains('\n') {
                    if !self.at_start_of_line {
                        self.trim_and_newline();
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
                self.push_indent();
            }
            self.push_content(&normalize_django(raw));
            self.emit_newline_or_continue(token, true);
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
            let actual_tag_name = strip_end_prefix(tag_name);
            let is_block = is_block_tag(actual_tag_name, &self.config.custom_blocks);
            let is_reindent = is_reindent_tag(tag_name);
            let is_line_break = is_line_break_tag(tag_name);

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

            if !self.at_start_of_line && (is_block || is_reindent || is_line_break) {
                self.trim_and_newline();
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

                        // djlint never collapses {% block %}...{% endblock name %}
                        // when the endblock tag carries the block name.
                        let end_raw = self.tokens[j].raw();
                        let end_has_name = {
                            let inner = end_raw
                                .trim_start_matches("{%")
                                .trim_end_matches("%}")
                                .trim();
                            // e.g. "endblock title" has 2+ words
                            inner.split_whitespace().count() > 1
                        };

                        // djlint never collapses custom block tags.
                        let is_custom_block = self
                            .config
                            .custom_blocks
                            .iter()
                            .any(|b| b.eq_ignore_ascii_case(actual_tag_name));

                        let is_collapsible_django_block = matches!(
                            actual_tag_name,
                            "if" | "for" | "block" | "with" | "asynceach" | "asyncall" | "comment"
                        );

                        if condensed_len < self.config.max_line_length
                            && (all_strictly_inline || non_whitespace_elements.len() <= 1)
                            && !end_has_name
                            && !is_custom_block
                            && is_collapsible_django_block
                        {
                            if self.at_start_of_line {
                                self.push_indent();
                            }
                            self.push_content(&normalized_start);
                            self.push_content(collapsed_content);
                            self.push_content(&normalized_end);
                            self.trim_and_newline();

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
                    self.push_indent();
                }
                self.push_content(&normalize_django(raw));

                if raw.contains("djlint:off") {
                    self.formatting_enabled = false;
                }

                self.emit_newline_or_continue(token, !is_block && !is_reindent && !is_line_break);

                if (!is_closing && is_block) || is_reindent {
                    let (_, closing_idx) = get_children_info(self.pos, &self.tokens);
                    let should_indent = closing_idx.is_some()
                        || is_reindent
                        || (!self.config.require_closed_blocks
                            && can_have_closing_tag(actual_tag_name, &self.config.custom_blocks));

                    if should_indent {
                        let already_incremented =
                            self.last_increment_line == Some(self.output_line_index);
                        let mut incremented = false;
                        if !already_incremented {
                            self.indent_level += 1;
                            self.last_increment_line = Some(self.output_line_index);
                            incremented = true;
                        }
                        self.parent_stack
                            .push((self.pos, incremented, false, false));
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
static DJANGO_ATTR_AFTER_RE: OnceLock<Regex> = OnceLock::new();
static DJANGO_ATTR_BEFORE_RE: OnceLock<Regex> = OnceLock::new();

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

static WHITESPACE_RE: OnceLock<Regex> = OnceLock::new();

fn format_tag(
    name: &str,
    raw: &str,
    is_self_closing: bool,
    indent_level: usize,
    spaces_before_tag: usize,
    config: &Config,
    is_ignored_block: bool,
) -> String {
    let attr_re_str = if config.better_attribute_parsing {
        r#"([a-zA-Z0-9:@._#*!-]+(?:\s*=\s*(?:"(?:\{\{[\s\S]*?\}\}|\{%[\s\S]*?%\}|[^"])*"|'(?:\{\{[\s\S]*?\}\}|\{%[\s\S]*?%\}|[^'])*'|[^\s>]+))?|\{\{[\s\S]*?\}\}|\{%[\s\S]*?%\})"#
    } else {
        r#"([a-zA-Z0-9:@._#*!-]+(?:\s*=\s*(?:"(?:(?:\{%-?\s*(?:if|for|asyncAll|asyncEach)[^\}]*?%\}(?:[\s\S]*?\{%\s*end(?:if|for|each|all)[^\}]*?-?%\})+?)|\{\{[\s\S]*?\}\}|\{%[\s\S]*?%\}|[^"])*"|'(?:(?:\{%-?\s*(?:if|for|asyncAll|asyncEach)[^\}]*?%\}(?:[\s\S]*?\{%\s*end(?:if|for|each|all)[^\}]*?-?%\})+?)|\{\{[\s\S]*?\}\}|\{%[\s\S]*?%\}|[^'])*'|[^\s>]+))?|(?:\{%-?\s*(?:if|for|asyncAll|asyncEach)[^\}]*?%\}(?:[\s\S]*?\{%\s*end(?:if|for|each|all)[^\}]*?-?%\})+?)|\{\{[\s\S]*?\}\}|\{%[\s\S]*?%\})"#
    };

    // In actual implementation we wouldn't want to recompile the regex
    // every time. However, to match the config option and debug, we'll
    // compile it here. Once fixed we can make it a static or put it in config.
    let attr_re = Regex::new(attr_re_str).unwrap();
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
                let normalized = normalize_django(raw_attr);
                whitespace_re.replace_all(&normalized, " ").to_string()
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
    if !is_ignored_block && prev_was_django_block {
        raw_final_content.push_str(filler.trim());
    } else {
        raw_final_content.push_str(filler);
    }

    // Collapse whitespace (e.g., multi-line attributes) to single spaces.
    // This string preserves style attribute values as-is (including trailing
    // semicolons), matching djlint which only reformats style when wrapping.
    let raw_attrs_collapsed = whitespace_re.replace_all(&raw_final_content, " ");

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
    let mut bytes = raw.as_bytes();
    if bytes.starts_with(b"{%") {
        bytes = &bytes[2..];
    }
    if bytes.starts_with(b"-") {
        bytes = &bytes[1..];
    }

    // Find start of word
    let mut start = 0;
    while start < bytes.len() && bytes[start].is_ascii_whitespace() {
        start += 1;
    }

    if start == bytes.len() {
        return None;
    }

    // Find end of word
    let mut end = start;
    while end < bytes.len()
        && !bytes[end].is_ascii_whitespace()
        && bytes[end] != b'%'
        && bytes[end] != b'-'
    {
        end += 1;
    }

    // SAFETY: We only advanced indices over ASCII characters (whitespace, '%', '-'),
    // so `start` and `end` are valid UTF-8 boundaries within `raw`.
    let start_idx = raw.len() - bytes.len() + start;
    let end_idx = raw.len() - bytes.len() + end;

    Some(&raw[start_idx..end_idx])
}

/// Strip the "end" prefix from a Django tag name (e.g., "endblock" -> "block").
/// Returns the original name if it doesn't start with "end".
fn strip_end_prefix(name: &str) -> &str {
    name.strip_prefix("end").unwrap_or(name)
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
            if is_django_block_self_closing(raw) {
                return true;
            }
            !is_block_tag(strip_end_prefix(tag_name), &config.custom_blocks)
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
            if is_line_break_tag(tag_name) {
                return false;
            }
            !is_block_tag(strip_end_prefix(tag_name), &config.custom_blocks)
        }
        Token::Text { raw, .. } => !raw.starts_with('\n') && !raw.starts_with("\r\n"),
        Token::Tag { name, .. } => is_inline_tag(name),
        Token::Comment { raw, .. } | Token::DjangoComment { raw, .. } => !raw.contains('\n'),
        Token::Doctype { .. } => false,
    }
}

fn is_block_tag(name: &str, custom_blocks: &[String]) -> bool {
    crate::tags::is_django_block_tag(name, custom_blocks)
}

/// Template tags that always occupy their own line but do NOT open an
/// indented block (no matching end-tag).  They are still collapsible by
/// `try_collapse_html_tag` when they are the sole content of a short
/// parent — matching djlint's two-phase expand-then-condense behaviour.
fn is_line_break_tag(name: &str) -> bool {
    matches!(name, "include")
}

/// Returns true for Cotton-style self-closing block tags (ending with `/ %}`).
/// These never open an indented block even when the tag name is in custom_blocks.
fn is_django_block_self_closing(raw: &str) -> bool {
    let s = raw
        .trim_end_matches('}')
        .trim_end_matches('%')
        .trim_end_matches('-')
        .trim_end_matches(' ');
    s.ends_with('/')
}

fn can_have_closing_tag(name: &str, custom_blocks: &[String]) -> bool {
    let name_lower = name.to_lowercase();
    let actual_name = name_lower.strip_prefix("end").unwrap_or(&name_lower);

    matches!(
        actual_name,
        "block"
            | "if"
            | "ifchanged"
            | "for"
            | "with"
            | "autoescape"
            | "filter"
            | "spaceless"
            | "cache"
            | "macro"
            | "call"
            | "set"
            | "localize"
            | "compress"
            | "comment"
            | "verbatim"
            | "language"
            | "thumbnail"
            | "raw"
    ) || custom_blocks
        .iter()
        .any(|b| b.eq_ignore_ascii_case(actual_name))
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
                    } if n.eq_ignore_ascii_case(name) => {
                        depth += 1;
                    }
                    Token::Tag {
                        name: n,
                        is_closing: true,
                        ..
                    } if n.eq_ignore_ascii_case(name) => {
                        depth -= 1;
                        if depth == 0 {
                            return (children, Some(j));
                        }
                    }
                    _ => {}
                }
                if depth == 1 {
                    children.push(j);
                }
                j += 1;
            }
            (children, None)
        }
        Token::DjangoBlock { raw, .. } => {
            let tag_name = get_django_tag_name(raw).unwrap_or("");
            let mut j = index + 1;
            let mut depth = 1;
            while j < tokens.len() {
                if let Token::DjangoBlock { raw: r, .. } = &tokens[j] {
                    let name = get_django_tag_name(r).unwrap_or("");
                    if name == tag_name {
                        depth += 1;
                    } else if name.starts_with("end") && &name[3..] == tag_name {
                        depth -= 1;
                        if depth == 0 {
                            return (children, Some(j));
                        }
                    }
                }
                if depth == 1 {
                    children.push(j);
                }
                j += 1;
            }
            (children, None)
        }
        _ => (children, None),
    }
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

            let is_ignored_block = matches!(
                name.to_lowercase().as_str(),
                "pre" | "textarea" | "script" | "style"
            );
            let formatted = format_tag(name, raw, *is_self_closing, 0, 0, config, is_ignored_block);
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
                let is_ignored_block = matches!(
                    name.to_lowercase().as_str(),
                    "pre" | "textarea" | "script" | "style"
                );
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
                                indent_level * config.indent,
                                config,
                                is_ignored_block,
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
                                indent_level * config.indent,
                                config,
                                is_ignored_block,
                            ));
                        }
                    } else {
                        result.push_str(&format_tag(
                            name,
                            raw,
                            *is_self_closing,
                            indent_level,
                            indent_level * config.indent,
                            config,
                            is_ignored_block,
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
