use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, PartialEq, Clone)]
pub enum Token<'a> {
    Tag {
        name: &'a str,
        raw: &'a str,
        is_closing: bool,
        is_self_closing: bool,
        line: usize,
        column: usize,
        offset: usize,
    },
    Comment {
        raw: &'a str,
        line: usize,
        column: usize,
        offset: usize,
    },
    DjangoComment {
        raw: &'a str,
        line: usize,
        column: usize,
        offset: usize,
    },
    Text {
        raw: &'a str,
        line: usize,
        column: usize,
        offset: usize,
    },
    Doctype {
        raw: &'a str,
        line: usize,
        column: usize,
        offset: usize,
    },
    DjangoVar {
        raw: &'a str,
        line: usize,
        column: usize,
        offset: usize,
    },
    DjangoBlock {
        raw: &'a str,
        line: usize,
        column: usize,
        offset: usize,
    },
}

pub struct Tokenizer<'a> {
    source: &'a str,
    pos: usize,
    line: usize,
    column: usize,
}

static TAG_RE: OnceLock<Regex> = OnceLock::new();
static CLOSE_TAG_RE: OnceLock<Regex> = OnceLock::new();
static COMMENT_RE: OnceLock<Regex> = OnceLock::new();
static DOCTYPE_RE: OnceLock<Regex> = OnceLock::new();
static DJANGO_VAR_RE: OnceLock<Regex> = OnceLock::new();
static DJANGO_BLOCK_RE: OnceLock<Regex> = OnceLock::new();
static DJANGO_COMMENT_RE: OnceLock<Regex> = OnceLock::new();

impl<'a> Tokenizer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            pos: 0,
            line: 1,
            column: 0,
        }
    }

    fn update_pos(&mut self, len: usize) {
        let consumed = &self.source[self.pos..self.pos + len];
        for c in consumed.chars() {
            if c == '\n' {
                self.line += 1;
                self.column = 0;
            } else {
                self.column += 1;
            }
        }
        self.pos += len;
    }
}

impl<'a> Iterator for Tokenizer<'a> {
    type Item = Token<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.source.len() {
            return None;
        }

        let remaining = &self.source[self.pos..];
        let current_line = self.line;
        let current_column = self.column;
        let current_offset = self.pos;

        let comment_re = COMMENT_RE.get_or_init(|| Regex::new(r#"(?i)^<!--[\s\S]*?-->"#).unwrap());
        if let Some(m) = comment_re.find(remaining) {
            let raw = m.as_str();
            self.update_pos(m.end());
            return Some(Token::Comment {
                raw,
                line: current_line,
                column: current_column,
                offset: current_offset,
            });
        }

        let django_var_re =
            DJANGO_VAR_RE.get_or_init(|| Regex::new(r#"^\{\{[\s\S]*?\}\}"#).unwrap());
        if let Some(m) = django_var_re.find(remaining) {
            let raw = m.as_str();
            self.update_pos(m.end());
            return Some(Token::DjangoVar {
                raw,
                line: current_line,
                column: current_column,
                offset: current_offset,
            });
        }

        let django_block_re =
            DJANGO_BLOCK_RE.get_or_init(|| Regex::new(r#"^\{%[\s\S]*?%\}"#).unwrap());
        if let Some(m) = django_block_re.find(remaining) {
            let raw = m.as_str();
            self.update_pos(m.end());
            return Some(Token::DjangoBlock {
                raw,
                line: current_line,
                column: current_column,
                offset: current_offset,
            });
        }

        let django_comment_re =
            DJANGO_COMMENT_RE.get_or_init(|| Regex::new(r#"^\{#[\s\S]*?#\}"#).unwrap());
        if let Some(m) = django_comment_re.find(remaining) {
            let raw = m.as_str();
            self.update_pos(m.end());
            return Some(Token::DjangoComment {
                raw,
                line: current_line,
                column: current_column,
                offset: current_offset,
            });
        }

        let doctype_re = DOCTYPE_RE.get_or_init(|| Regex::new(r#"(?i)^<!DOCTYPE[^>]*>"#).unwrap());
        if let Some(m) = doctype_re.find(remaining) {
            let raw = m.as_str();
            self.update_pos(m.end());
            return Some(Token::Doctype {
                raw,
                line: current_line,
                column: current_column,
                offset: current_offset,
            });
        }

        // Close tags carry no attributes, so they must be `</name ...>` with
        // the `>` on the same line. Matching them with a dedicated regex stops
        // the general tag regex below from swallowing a *malformed* close tag
        // (`</div` with no `>`) across newlines, template tags, and following
        // tags until some distant `>` — which would drop the swallowed content.
        // A malformed close tag matches just `</name` and is preserved as-is
        // (no `>` added), mirroring djlint which leaves it literal.
        if remaining.starts_with("</") {
            let close_re = CLOSE_TAG_RE
                .get_or_init(|| Regex::new(r#"(?i)^</([a-z0-9:._-]+)([^>\n]*>)?"#).unwrap());
            if let Some(caps) = close_re.captures(remaining) {
                let m = caps.get(0).unwrap();
                let name = caps.get(1).unwrap().as_str();
                let raw = m.as_str();
                self.update_pos(m.end());
                return Some(Token::Tag {
                    name,
                    raw,
                    is_closing: true,
                    is_self_closing: false,
                    line: current_line,
                    column: current_column,
                    offset: current_offset,
                });
            }
        }

        let tag_re = TAG_RE.get_or_init(|| Regex::new(
            r#"(?i)^</?([a-z0-9:._-]+)(?:"[^"]*"|'[^']*'|\{\{[\s\S]*?\}\}|\{%[\s\S]*?%\}|\{#[\s\S]*?#\}|[^>{}'"])*>"#,
        ).unwrap());
        if let Some(m) = tag_re.find(remaining) {
            let caps = tag_re.captures(remaining).unwrap();
            let name = caps.get(1).unwrap().as_str();
            let raw = m.as_str();
            let is_closing = raw.starts_with("</");
            let is_self_closing = raw.ends_with("/>") || crate::is_void_element(name);

            self.update_pos(m.end());
            return Some(Token::Tag {
                name,
                raw,
                is_closing,
                is_self_closing,
                line: current_line,
                column: current_column,
                offset: current_offset,
            });
        }

        // If no match, it's text until the next '<' or '{'
        let mut next_stop = remaining.len();
        for (i, c) in remaining.char_indices() {
            if i > 0
                && (c == '<'
                    || (c == '{'
                        && (remaining[i..].starts_with("{{")
                            || remaining[i..].starts_with("{%")
                            || remaining[i..].starts_with("{#"))))
            {
                next_stop = i;
                break;
            }
        }

        let raw = &remaining[..next_stop];
        self.update_pos(next_stop);
        Some(Token::Text {
            raw,
            line: current_line,
            column: current_column,
            offset: current_offset,
        })
    }
}

impl<'a> Token<'a> {
    pub fn raw(&self) -> &'a str {
        match self {
            Token::Tag { raw, .. } => raw,
            Token::Comment { raw, .. } => raw,
            Token::DjangoComment { raw, .. } => raw,
            Token::Text { raw, .. } => raw,
            Token::Doctype { raw, .. } => raw,
            Token::DjangoVar { raw, .. } => raw,
            Token::DjangoBlock { raw, .. } => raw,
        }
    }

    pub fn line(&self) -> usize {
        match self {
            Token::Tag { line, .. } => *line,
            Token::Comment { line, .. } => *line,
            Token::DjangoComment { line, .. } => *line,
            Token::Text { line, .. } => *line,
            Token::Doctype { line, .. } => *line,
            Token::DjangoVar { line, .. } => *line,
            Token::DjangoBlock { line, .. } => *line,
        }
    }

    pub fn column(&self) -> usize {
        match self {
            Token::Tag { column, .. } => *column,
            Token::Comment { column, .. } => *column,
            Token::DjangoComment { column, .. } => *column,
            Token::Text { column, .. } => *column,
            Token::Doctype { column, .. } => *column,
            Token::DjangoVar { column, .. } => *column,
            Token::DjangoBlock { column, .. } => *column,
        }
    }

    pub fn offset(&self) -> usize {
        match self {
            Token::Tag { offset, .. } => *offset,
            Token::Comment { offset, .. } => *offset,
            Token::DjangoComment { offset, .. } => *offset,
            Token::Text { offset, .. } => *offset,
            Token::Doctype { offset, .. } => *offset,
            Token::DjangoVar { offset, .. } => *offset,
            Token::DjangoBlock { offset, .. } => *offset,
        }
    }

    pub fn ends_on_line(&self) -> usize {
        self.line() + self.raw().chars().filter(|&c| c == '\n').count()
    }
}
