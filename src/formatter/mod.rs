pub mod tokenizer;

use crate::config::Config;
use crate::tags::{
    is_break_html_tag, is_condensable_tag, is_html_block_tag, is_inline_tag, is_structural_tag,
    should_indent_children, should_wrap_attributes,
};
use regex::Regex;
use std::sync::OnceLock;
use tokenizer::{Token, Tokenizer};

mod predicates;
mod tag_format;
mod tree;

use predicates::*;
use tag_format::*;
use tree::*;

/// Pre-scan `tokens` to find which `<pre>` / `<textarea>` openings have no
/// matching close tag.  These unclosed verbatim blocks need special treatment:
/// djlint's `clean_whitespace` step strips leading indentation from content
/// outside properly-closed ignored blocks, so we replicate that behaviour by
/// stripping whitespace-only text tokens when inside such a block.
fn find_unclosed_verbatim_positions(tokens: &[Token<'_>]) -> std::collections::HashSet<usize> {
    let mut open_stack: Vec<(usize, String)> = Vec::new();
    for (i, token) in tokens.iter().enumerate() {
        if let Token::Tag {
            name,
            is_closing,
            is_self_closing,
            ..
        } = token
        {
            let lower = name.to_lowercase();
            if matches!(lower.as_str(), "pre" | "textarea") && !is_self_closing {
                if !is_closing {
                    open_stack.push((i, lower));
                } else if let Some(j) = open_stack.iter().rposition(|(_, n)| n == &lower) {
                    open_stack.remove(j);
                }
            }
        }
    }
    open_stack.into_iter().map(|(pos, _)| pos).collect()
}

/// Whether a tag's raw text contains an unquoted `{{ … }}` attribute value
/// (e.g. `<div id={{ x }} …>`). djlint's "break after an html tag" regex
/// matches an unquoted attribute value with `{[^}]*}`, which consumes only the
/// first `}` of `}}` and leaves a stray `}` — so the tag fails to match and the
/// content after `>` is never broken onto its own line. (Quoted `{{ }}` and
/// unquoted `{% %}` do not trip this.) We mirror that by keeping such a tag's
/// source-inline content on the opening tag's line.
fn has_unquoted_double_brace(raw: &str) -> bool {
    let bytes = raw.as_bytes();
    let mut quote: Option<u8> = None;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        match quote {
            Some(q) => {
                if b == q {
                    quote = None;
                }
            }
            None => {
                if b == b'"' || b == b'\'' {
                    quote = Some(b);
                } else if b == b'{' && i + 1 < bytes.len() && bytes[i + 1] == b'{' {
                    return true;
                }
            }
        }
        i += 1;
    }
    false
}

/// Pre-scan for djlint's "template tag inside a tag's attributes poisons the
/// global move-decision" quirk.  djlint's expand step decides whether to break
/// a template tag (`{% endif %}` …) onto its own line by searching the whole
/// document so far for `<indent_html_tag … {% sametag %}$`.  When an opening
/// `indent_html_tags` tag carries a template block tag at the end of one of its
/// attribute lines (e.g. inside a `{# djlint:off #}` region), that search
/// matches for every *later* occurrence of the same tag text — so djlint stops
/// breaking those tags onto their own line.  We mirror this by recording, for
/// each such tag, its token position and the normalised text of the embedded
/// template tags, then keeping a matching inline closer attached to its line.
fn find_poison_closers(tokens: &[Token<'_>]) -> Vec<(usize, String)> {
    let mut out: Vec<(usize, String)> = Vec::new();
    let tag_re = BLOCK_RE.get_or_init(|| Regex::new(r#"\{%[\s\S]*?%\}"#).unwrap());
    for (i, token) in tokens.iter().enumerate() {
        if let Token::Tag {
            name,
            raw,
            is_closing: false,
            is_self_closing: false,
            ..
        } = token
        {
            if !crate::tags::is_indent_html_tag(name) {
                continue;
            }
            // A template tag counts only if it sits at the end of a line within
            // the opening tag (followed by optional spaces then a newline),
            // mirroring djlint's `… %}$` anchor.
            for m in tag_re.find_iter(raw) {
                let after = &raw[m.end()..];
                let trimmed = after.trim_start_matches([' ', '\t']);
                if trimmed.starts_with('\n') || trimmed.starts_with("\r\n") {
                    out.push((i, normalize_django(m.as_str())));
                }
            }
        }
    }
    out
}

/// djlint disposition of a `{# djlint:off #}` marker found inside an opening
/// tag's attributes. Computed once from the tag's raw text so the two handling
/// paths can't drift apart on independent substring checks.
enum OffRegion {
    /// No off marker — handle the tag normally.
    None,
    /// A complete `{# djlint:off #}` … `{# djlint:on #}` region: emit the tag
    /// preserving its source line breaks, then handle content/close normally.
    SelfContained,
    /// `{# djlint:off #}` with no matching `{# djlint:on #}` in the tag: the
    /// off region extends past the tag, so emit verbatim and disable formatting
    /// until a later `{# djlint:on #}`.
    Opens,
}

fn off_region_in_tag(raw: &str) -> OffRegion {
    if !raw.contains("djlint:off") {
        OffRegion::None
    } else if raw.contains("djlint:on") {
        OffRegion::SelfContained
    } else {
        OffRegion::Opens
    }
}

struct Formatter<'a> {
    config: &'a Config,
    output: String,
    indent_level: usize,
    tokens: Vec<Token<'a>>,
    pos: usize,
    formatting_enabled: bool,
    verbatim_tags: Vec<String>,
    /// True when the current (top) verbatim block was never closed in the source.
    /// djlint's clean_whitespace strips leading indentation for such blocks, so
    /// we replicate that by dropping spaces/tabs from whitespace-only text tokens.
    verbatim_is_unclosed: bool,
    /// Positions (in `tokens`) of verbatim-tag opens that have no matching close.
    unclosed_verbatim_positions: std::collections::HashSet<usize>,
    /// (token_pos, normalized_template_tag_text) of template tags embedded at a
    /// line end inside an opening tag's attributes. See `find_poison_closers`:
    /// a later closing template tag with the same text is kept inline rather
    /// than broken onto its own line.
    poison_closers: Vec<(usize, String)>,
    at_start_of_line: bool,
    /// Stack of (token_pos, incremented, tag_was_wrapped, was_inline_mid_line).
    parent_stack: Vec<(usize, bool, bool, bool)>,
    last_increment_line: Option<usize>,
    last_decrement_line: Option<usize>,
    output_line_index: usize,
    /// (source_line, tag_name) of the most recent inline-tag collapse.
    last_inline_collapse: Option<(usize, &'a str)>,
}

impl<'a> Formatter<'a> {
    fn new(config: &'a Config, source: &'a str) -> Self {
        let tokens: Vec<Token> = Tokenizer::new(source).collect();
        let unclosed_verbatim_positions = find_unclosed_verbatim_positions(&tokens);
        let poison_closers = find_poison_closers(&tokens);
        Self {
            config,
            output: String::new(),
            indent_level: 0,
            tokens,
            pos: 0,
            formatting_enabled: true,
            verbatim_tags: Vec::new(),
            verbatim_is_unclosed: false,
            unclosed_verbatim_positions,
            poison_closers,
            at_start_of_line: true,
            parent_stack: Vec::new(),
            last_increment_line: None,
            last_decrement_line: None,
            output_line_index: 0,
            last_inline_collapse: None,
        }
    }

    fn push_newline(&mut self) {
        self.output.push('\n');
        self.output_line_index += 1;
        self.at_start_of_line = true;
    }

    /// Whether the closing template tag at the current position is "poisoned"
    /// (see `find_poison_closers`): an earlier opening tag embedded a template
    /// tag with the same normalised text at a line end inside its attributes.
    /// When so, djlint keeps this closer attached to the preceding inline
    /// content rather than breaking it onto its own line.
    fn is_poisoned_closer(&self, raw: &str) -> bool {
        if self.poison_closers.is_empty() {
            return false;
        }
        let text = normalize_django(raw);
        self.poison_closers
            .iter()
            .any(|(pos, t)| *pos < self.pos && *t == text)
    }

    /// Whether the current whitespace-only text token sits between inline
    /// content and a poisoned closer (e.g. the `  ` in `</a>  {% endif %}`).
    /// djlint keeps the closer on that line, so the spacing must be preserved
    /// instead of being dropped as inter-tag whitespace.
    fn is_space_before_poisoned_closer(&self, raw: &str) -> bool {
        raw.trim().is_empty()
            && !self.at_start_of_line
            && !raw.contains('\n')
            && self.pos + 1 < self.tokens.len()
            && matches!(&self.tokens[self.pos + 1], Token::DjangoBlock { .. })
            && self.is_poisoned_closer(self.tokens[self.pos + 1].raw())
    }

    /// Emit an opening tag that carries a `{# djlint:off #}` marker in its
    /// attributes. Returns `true` when handled here (caller should stop),
    /// `false` to fall through to normal tag handling. See `OffRegion`.
    fn try_emit_djlint_off_open_tag(&mut self, name: &str, raw: &str) -> bool {
        let region = off_region_in_tag(raw);
        if matches!(region, OffRegion::None) {
            return false;
        }

        // Shared preamble: a block tag mid-line starts a fresh line, then indent.
        if !self.at_start_of_line && is_html_block_tag(name) {
            self.trim_and_newline();
        }
        if self.at_start_of_line {
            self.push_indent();
        }

        match region {
            OffRegion::None => unreachable!(),
            OffRegion::SelfContained => {
                // Preserve the source line breaks: lines strictly between the
                // markers stay verbatim; every other line (marker lines, the
                // closing `>`, other attributes) is re-indented one level deeper
                // than the tag.
                let cont_indent = " ".repeat((self.indent_level + 1) * self.config.indent);
                let mut in_off = false;
                for (i, line) in raw.split('\n').enumerate() {
                    if i == 0 {
                        self.push_content(line);
                        continue;
                    }
                    self.push_newline();
                    let stripped = line.trim_start();
                    if in_off && !stripped.starts_with("{# djlint:on") {
                        self.output.push_str(line);
                    } else {
                        self.output.push_str(&cont_indent);
                        self.output.push_str(stripped.trim_end());
                        if stripped.starts_with("{# djlint:off") {
                            in_off = true;
                        } else if stripped.starts_with("{# djlint:on") {
                            in_off = false;
                        }
                    }
                }
                self.at_start_of_line = false;
                // The tag opened normally: newline before its content, increment,
                // and track the parent so the matching close decrements.
                self.trim_and_newline();
                let incremented = if should_indent_children(name) {
                    self.increment_indent(true)
                } else {
                    false
                };
                self.parent_stack.push((self.pos, incremented, true, false));
            }
            OffRegion::Opens => {
                self.push_content(raw);
                // When the marker sits on a *later* line than the tag's opening
                // (e.g. `<a class="x"` then a newline then the marker), djlint
                // processes that opening-tag line normally and an
                // `indent_html_tags` opener increments the indent. The matching
                // close is then consumed verbatim inside the off block, so the
                // increment is never reversed and "leaks" one level. When the
                // marker is on the opener's own line (e.g.
                // `<div {# djlint:off #}>`), djlint never counts the increment.
                let off_pos = raw.find("djlint:off").unwrap_or(0);
                if raw[..off_pos].contains('\n') && crate::tags::is_indent_html_tag(name) {
                    self.indent_level += 1;
                }
                self.formatting_enabled = false;
                self.at_start_of_line = false;
            }
        }
        true
    }

    /// Increment the indent level, honoring djlint's "at most one indent
    /// change per output line" rule.  When `force` is true the dedup check is
    /// skipped (e.g. the child starts on a freshly-emitted line).  Returns
    /// `true` if the increment was actually applied.
    fn increment_indent(&mut self, force: bool) -> bool {
        if !force && self.last_increment_line == Some(self.output_line_index) {
            return false;
        }
        self.indent_level += 1;
        self.last_increment_line = Some(self.output_line_index);
        true
    }

    /// Decrement the indent level (saturating), honoring djlint's "at most one
    /// indent change per output line" rule.
    fn decrement_indent(&mut self) {
        if self.last_decrement_line == Some(self.output_line_index) {
            return;
        }
        self.indent_level = self.indent_level.saturating_sub(1);
        self.last_decrement_line = Some(self.output_line_index);
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

    fn handle_tag(&mut self, token: &Token<'a>) {
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
                    self.verbatim_is_unclosed = false;
                    let is_closing_verbatim = was_verbatim_name.is_some();

                    let was_incremented = self
                        .parent_stack
                        .pop()
                        .map(|(_, inc, _, _)| inc)
                        .unwrap_or(false);

                    if was_incremented {
                        self.decrement_indent();
                    }

                    if !self.at_start_of_line && !is_inline_tag(name) && !is_closing_verbatim {
                        self.trim_and_newline();
                    }
                    if self.at_start_of_line {
                        // For </script> and </style>, trim source whitespace
                        // and re-indent to the computed level. For </textarea>
                        // and </pre>, the block is truly raw — djlint preserves
                        // whatever indentation the source had before the closing
                        // tag, so we leave the output whitespace untouched.
                        let is_truly_verbatim = matches!(name_lower.as_str(), "pre" | "textarea");
                        if !is_truly_verbatim {
                            if is_closing_verbatim {
                                trim_trailing_whitespace(&mut self.output);
                            }
                            self.push_indent();
                        }
                    }
                    self.push_content(&format!("</{}>", name));
                    self.emit_newline_or_continue(
                        token,
                        is_inline_tag(name) || is_closing_verbatim,
                    );
                    return;
                } else {
                    // djlint's collapse_whitespace pre-pass normalises
                    // tags everywhere, including inside verbatim blocks:
                    // multi-line attribute lists are folded to one line.
                    let tag_content: std::borrow::Cow<str> =
                        if raw.contains('\n') || raw.contains('\r') {
                            let ws_re = WHITESPACE_RE.get_or_init(|| Regex::new(r"\s+").unwrap());
                            std::borrow::Cow::Owned(ws_re.replace_all(raw, " ").into_owned())
                        } else {
                            std::borrow::Cow::Borrowed(raw)
                        };
                    self.push_content(&tag_content);
                    self.at_start_of_line = tag_content.ends_with('\n');
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

                // djlint treats a line that starts with <tag> and ends with
                // </tag> (SAME tag name) as net-0, even when the inner pair
                // also closes immediately.  An extra closer whose name
                // matches the most-recently-collapsed inline tag on the same
                // source line should therefore not decrement.
                let is_extra_closer_after_collapse = !open_close_same_line
                    && self.last_inline_collapse.is_some_and(|(line, tag)| {
                        line == token.line() && tag.to_lowercase() == name.to_lowercase()
                    });

                let should_decrement = if open_close_same_line && was_incremented {
                    true
                } else if is_extra_closer_after_collapse {
                    false
                } else {
                    (was_incremented
                        || (was_inline_mid_line
                            && should_indent_children(name)
                            && self.at_start_of_line))
                        && !closing_midline_with_trailing
                };
                if should_decrement {
                    self.decrement_indent();
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
                let force_newline = !is_inline_tag(name) || is_break_before_close_tag(name);

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
                self.emit_newline_or_continue(
                    token,
                    is_inline_tag(name) && !is_break_before_close_tag(name),
                );
            } else {
                // A `{# djlint:off #}` marker inside an opening tag's
                // attributes is handled out-of-line (see `OffRegion`); a
                // self-closing tag never matches, matching djlint.
                if !is_self_closing && self.try_emit_djlint_off_open_tag(name, raw) {
                    return;
                }

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
                // An "unclosed orphan" only applies to non-block tags (e.g. SVG
                // shape elements like <circle> that are not in djlint's
                // `indent_html_tags`).  Standard HTML block tags like <td> ARE
                // in that set and always increment — even when unclosed — so we
                // must NOT suppress their increment here.
                let is_unclosed_orphan = mismatched_void_close.is_none()
                    && closing_idx.is_none()
                    && !is_self_closing
                    && !is_html_block_tag(name)
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

                // djlint wraps textarea attributes when the opening and closing
                // tags are on the same source line (regardless of content).
                // When the closing tag is on a separate line, djlint treats
                // the opening tag as an ignored-block opener and skips
                // format_attributes entirely.
                let textarea_is_inline = name_lower == "textarea"
                    && !is_self_closing
                    && closing_idx
                        .map(|j| self.tokens[j].line() == token.ends_on_line())
                        .unwrap_or(false);

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
                    textarea_is_inline,
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
                        self.verbatim_is_unclosed =
                            self.unclosed_verbatim_positions.contains(&self.pos);
                    }

                    let tag_was_wrapped = formatted_tag.contains('\n');

                    if !is_verbatim {
                        // Allow inline continuation only for non-structural,
                        // non-wrapped tags that are inline or non-block.
                        // However, inline tags like <a> should NOT be forced
                        // to wrap their content just because their attributes wrapped.
                        // A tag with an unquoted `{{ … }}` attribute also keeps
                        // its source-inline content on the opening tag's line:
                        // djlint's break-after regex can't match such a tag (see
                        // `has_unquoted_double_brace`). The same-line check in
                        // `should_continue_inline` still gates this.
                        let unquoted_double_brace = has_unquoted_double_brace(raw);
                        let inline_guard = !is_structural_tag(name)
                            && (!tag_was_wrapped || is_inline_tag(name) || unquoted_double_brace)
                            && (is_inline_tag(name)
                                || !is_html_block_tag(name)
                                || unquoted_double_brace);
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
                        // Force (skip dedup) when:
                        // - we just pushed a newline (children on a new line)
                        // - tag was at start of line (tag is a child that
                        //   should increment independently of its parent)
                        incremented = self.increment_indent(pushed_newline || was_at_start_of_line);
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
        name: &'a str,
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
            if non_whitespace_elements.is_empty() {
                // An empty element's closing tag stays inline unless djlint's
                // expand step would break it onto its own line — i.e. the tag
                // is in break_html_tags and is not pulled back by the condense
                // step (optional_single_line_html_tags / is_condensable_tag).
                // Tags like canvas/noscript are block-level but absent from
                // break_html_tags, so their close stays inline.
                is_condensable_tag(name) || !is_break_html_tag(name)
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
                // Determine whether all non-whitespace content tokens reside on
                // the same source line.  This distinguishes:
                //   <p>\n{{ x }} text\n</p>  — content on one line, collapsible
                //   <li>{{ a }}:\n{{ b }}</li> — content spans lines, do NOT collapse
                let all_nonws_same_line = {
                    let tokens = &self.tokens;
                    let mut first_line: Option<usize> = None;
                    let mut ok = true;
                    'check: for elem in &non_whitespace_elements {
                        for tok in tokens.iter().take(elem.end).skip(elem.start) {
                            if let Token::Text { raw, .. } = tok {
                                if raw.trim().is_empty() {
                                    continue;
                                }
                            }
                            // Use only the start line — trailing \n in text
                            // tokens would push ends_on_line past the content
                            // line and give a false "multi-line" signal.
                            let sl = tok.line();
                            match first_line {
                                None => {
                                    first_line = Some(sl);
                                }
                                Some(fl) if sl != fl => {
                                    ok = false;
                                    break 'check;
                                }
                                _ => {}
                            }
                        }
                    }
                    ok
                };
                ((same_line && !has_newline_text) || (!has_content_newline && all_nonws_same_line))
                    && !has_any_tag
            }
        } else if has_any_tag {
            !has_newline_text
        } else {
            // For non-block elements whose only "content" is whitespace spanning
            // multiple source lines (e.g. <circle>\n</circle>), apply the same
            // condensable-tag gate that block elements use.  djlint only condenses
            // tags that appear in optional_single_line_html_tags (is_condensable_tag).
            if non_whitespace_elements.is_empty() && has_newline_text {
                is_condensable_tag(name)
            } else {
                !has_newline_text || non_whitespace_elements.len() == 1
            }
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
        // Use char counts (not byte lengths) to match djlint's Python
        // len(), which counts Unicode code points, not UTF-8 bytes.
        let tag_last_line_len = formatted_tag
            .split('\n')
            .next_back()
            .unwrap_or("")
            .chars()
            .count();
        let current_line_len = if formatted_tag.contains('\n') {
            tag_last_line_len
        } else {
            (self.indent_level * self.config.indent) + tag_last_line_len
        };
        let projected_len = current_line_len + collapsed_content.chars().count() + name.len() + 3;

        // djlint calculates combined length for condensation using regex:
        // len(last_line_of_open_tag + content + close_tag)
        // Which effectively ignores the indentation of the open tag
        // if the open tag is a single line, and only considers the last line
        // if it's multiline.
        let djlint_condensed_len =
            tag_last_line_len + collapsed_content.chars().count() + name.len() + 3;

        // djlint ignores the outer indentation when deciding whether to
        // collapse: it checks condensed length (tag last line + content +
        // close tag) rather than the full projected line length.  Apply
        // the same skip for any element whose content has no child HTML
        // tags (only text/template vars/whitespace).
        let skip_line_length_check =
            !has_any_tag && djlint_condensed_len < self.config.max_line_length;

        if projected_len < self.config.max_line_length
            || is_potentially_verbatim
            || skip_line_length_check
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
            if is_inline_tag(name) {
                self.last_inline_collapse = Some((self.tokens[j].line(), name));
            }
            self.emit_newline_or_continue(token, is_inline_tag(name));
            true
        } else {
            false
        }
    }

    fn handle_text(&mut self, token: &Token) {
        let raw = token.raw();
        if !self.verbatim_tags.is_empty() {
            // djlint's clean_whitespace strips leading indentation from content
            // in unclosed verbatim blocks (e.g. an unclosed <pre>) because its
            // ignored-block regex requires a matching close tag.  Replicate that
            // by dropping spaces/tabs from whitespace-only inter-tag tokens.
            if self.verbatim_is_unclosed && raw.trim().is_empty() {
                let stripped: String = raw.chars().filter(|&c| c == '\n' || c == '\r').collect();
                self.push_content(&stripped);
            } else {
                self.push_content(raw);
            }
            // Don't override at_start_of_line — push_content already
            // tracks this correctly based on the content's newlines.
        } else {
            let trimmed = raw.trim();
            // Whitespace separating inline content from a poisoned closer (e.g.
            // the `  ` in `</a>  {% endif %}`) is normally dropped, but djlint
            // keeps the closer on that line, so preserve the original spacing.
            if self.is_space_before_poisoned_closer(raw) {
                self.push_content(raw);
                return;
            }
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
                        } if is_inline_tag(n) && !is_break_before_close_tag(n)
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
                    // Skip pure-whitespace text immediately before a closing
                    // inline tag on the same line (djlint condense_html strips
                    // trailing whitespace between inline content and closers).
                    let next_is_closing_inline = self.pos + 1 < self.tokens.len()
                        && matches!(
                            &self.tokens[self.pos + 1],
                            Token::Tag {
                                name: n,
                                is_closing: true,
                                ..
                            } if is_inline_tag(n)
                        );
                    if !next_is_closing_inline {
                        self.push_content(raw);
                    }
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
                // When this source line ends with an unmatched inline close
                // tag (net-negative inline balance), djlint emits the whole
                // line at the parent's (pre-decremented) indent level.
                let dedent = line_ends_with_net_inline_close(&self.tokens, self.pos)
                    && self
                        .parent_stack
                        .last()
                        .is_some_and(|&(_, inc, tw, _)| inc && !tw);
                self.push_indent_maybe_dedented(dedent);
            }
            self.push_content(&normalize_django(raw));
            self.emit_newline_or_continue(token, true);
        }
    }

    fn handle_django_block(&mut self, token: &Token) {
        let raw = token.raw();
        if !self.verbatim_tags.is_empty() {
            // Check if this closes a verbatim Django block (e.g. {% endcomment %}).
            // These are Django blocks pushed onto verbatim_tags, not HTML tags, so
            // handle_tag's close logic never fires for them.
            let django_verbatim_blocks = ["comment"];
            let tag_name_for_close = get_django_tag_name(raw).unwrap_or("");
            if let Some(actual) = tag_name_for_close.strip_prefix("end") {
                if django_verbatim_blocks.contains(&actual)
                    && self.verbatim_tags.last().map(String::as_str) == Some(actual)
                {
                    self.verbatim_tags.pop();
                    // Only indent when content already ended on its own line.
                    // For inline comments ({% comment %}text{% endcomment %})
                    // the closing tag stays on the same line as the content.
                    if self.at_start_of_line {
                        // Verbatim content may leave trailing whitespace on the
                        // current line (e.g. the source indentation before the
                        // closing tag).  Strip it so our indent replaces it cleanly.
                        trim_trailing_whitespace(&mut self.output);
                        self.push_indent();
                    }
                    self.push_content(&normalize_django(raw));
                    self.trim_and_newline();
                    return;
                }
            }

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

            // {% comment %} is verbatim — content is preserved as-is without
            // indenting or reformatting, matching Python djlint's behavior.
            if tag_name == "comment" {
                if !self.at_start_of_line {
                    self.trim_and_newline();
                }
                if self.at_start_of_line {
                    self.push_indent();
                }
                self.push_content(&normalize_django(raw));
                // Don't emit a trailing newline — the verbatim content starts
                // with \n and provides its own line break.
                self.at_start_of_line = false;
                self.verbatim_tags.push("comment".to_string());
                return;
            }

            // A poisoned closer (e.g. `{% endif %}` after `</a>` on the same
            // source line) stays attached to the preceding inline content
            // instead of being broken onto its own line — matching djlint's
            // global move-decision quirk (see `find_poison_closers`). djlint
            // also never reverses the matching opener's indent in this case, so
            // the indent "leaks" one level: pop the parent but skip the
            // decrement.
            let poisoned_inline =
                is_closing && is_block && !self.at_start_of_line && self.is_poisoned_closer(raw);

            if (is_closing && is_block) || is_reindent {
                self.parent_stack.pop();
                if !poisoned_inline {
                    self.decrement_indent();
                }
            }

            // Check if we can inline
            let mut did_collapse = false;

            if !poisoned_inline
                && !self.at_start_of_line
                && (is_block || is_reindent || is_line_break)
            {
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
                            "if" | "for" | "block" | "with" | "asynceach" | "asyncall"
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
                            // Use local_depth so that extra closing tags
                            // (e.g. a second </span> inside a same-line
                            // {% if %}...{% endif %} block) do not
                            // decrement the indent below what the block's
                            // own opens warrant.
                            let mut local_depth: usize = 0;
                            for &child_idx in &children {
                                match &self.tokens[child_idx] {
                                    Token::Tag {
                                        name: n,
                                        is_closing: false,
                                        is_self_closing: false,
                                        ..
                                    } if is_inline_tag(n) || is_html_block_tag(n) => {
                                        local_depth += 1;
                                        self.indent_level += 1;
                                    }
                                    Token::Tag {
                                        name: n,
                                        is_closing: true,
                                        ..
                                    } if (is_inline_tag(n) || is_html_block_tag(n))
                                        && local_depth > 0 =>
                                    {
                                        local_depth -= 1;
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
                        let incremented = self.increment_indent(false);
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
