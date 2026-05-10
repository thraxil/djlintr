use regex::Regex;

#[derive(Debug, PartialEq, Clone)]
pub enum Token<'a> {
    Tag {
        name: &'a str,
        raw: &'a str,
        is_closing: bool,
        is_self_closing: bool,
        line: usize,
        column: usize,
    },
    Comment {
        raw: &'a str,
        line: usize,
        column: usize,
    },
    Text {
        raw: &'a str,
        line: usize,
        column: usize,
    },
    Doctype {
        raw: &'a str,
        line: usize,
        column: usize,
    },
    DjangoVar {
        raw: &'a str,
        line: usize,
        column: usize,
    },
    DjangoBlock {
        raw: &'a str,
        line: usize,
        column: usize,
    },
}

pub struct Tokenizer<'a> {
    source: &'a str,
    pos: usize,
    line: usize,
    column: usize,
    tag_re: Regex,
    comment_re: Regex,
    doctype_re: Regex,
    django_var_re: Regex,
    django_block_re: Regex,
}

impl<'a> Tokenizer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            pos: 0,
            line: 1,
            column: 0,
            tag_re: Regex::new(r#"(?i)^</?([a-z0-9:]+)[^>]*>"#).unwrap(),
            comment_re: Regex::new(r#"(?i)^<!--[\s\S]*?-->"#).unwrap(),
            doctype_re: Regex::new(r#"(?i)^<!DOCTYPE[^>]*>"#).unwrap(),
            django_var_re: Regex::new(r#"^\{\{[\s\S]*?\}\}"#).unwrap(),
            django_block_re: Regex::new(r#"^\{%[\s\S]*?%\}"#).unwrap(),
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

        if let Some(m) = self.comment_re.find(remaining) {
            let raw = m.as_str();
            self.update_pos(m.end());
            return Some(Token::Comment {
                raw,
                line: current_line,
                column: current_column,
            });
        }

        if let Some(m) = self.django_var_re.find(remaining) {
            let raw = m.as_str();
            self.update_pos(m.end());
            return Some(Token::DjangoVar {
                raw,
                line: current_line,
                column: current_column,
            });
        }

        if let Some(m) = self.django_block_re.find(remaining) {
            let raw = m.as_str();
            self.update_pos(m.end());
            return Some(Token::DjangoBlock {
                raw,
                line: current_line,
                column: current_column,
            });
        }

        if let Some(m) = self.doctype_re.find(remaining) {
            let raw = m.as_str();
            self.update_pos(m.end());
            return Some(Token::Doctype {
                raw,
                line: current_line,
                column: current_column,
            });
        }

        if let Some(m) = self.tag_re.find(remaining) {
            let caps = self.tag_re.captures(remaining).unwrap();
            let name = caps.get(1).unwrap().as_str();
            let raw = m.as_str();
            let is_closing = raw.starts_with("</");
            let is_self_closing = raw.ends_with("/>") || is_void_element(name);

            self.update_pos(m.end());
            return Some(Token::Tag {
                name,
                raw,
                is_closing,
                is_self_closing,
                line: current_line,
                column: current_column,
            });
        }

        // If no match, it's text until the next '<' or '{'
        let mut next_stop = remaining.len();
        for (i, c) in remaining.char_indices() {
            if i > 0 && (c == '<' || c == '{') {
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
        })
    }
}

fn is_void_element(name: &str) -> bool {
    matches!(
        name.to_lowercase().as_str(),
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

impl<'a> Token<'a> {
    pub fn raw(&self) -> &'a str {
        match self {
            Token::Tag { raw, .. } => raw,
            Token::Comment { raw, .. } => raw,
            Token::Text { raw, .. } => raw,
            Token::Doctype { raw, .. } => raw,
            Token::DjangoVar { raw, .. } => raw,
            Token::DjangoBlock { raw, .. } => raw,
        }
    }
}
