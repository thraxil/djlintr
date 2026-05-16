use crate::config::Config;
use crate::formatter::tokenizer::{Token, Tokenizer};
use regex::Regex;
use serde::{Deserialize, Serialize};

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

    let mut open_tags: Vec<(String, usize, usize, usize)> = Vec::new();
    let single_quote_attr_re = Regex::new(
        r#"(?i)\b(?:class|id|src|width|height|alt|style|lang|title|srcset|media)='[^']*'"#,
    )
    .unwrap();

    struct IgnoredRange {
        start: usize,
        end: usize,
        rules: Vec<String>,
    }

    let mut ignored_ranges: Vec<IgnoredRange> = Vec::new();

    let always_ignored_re = Regex::new(
        r#"(?is)<script.*?</script>|<style.*?</style>|<pre.*?</pre>|<textarea.*?</textarea>|<!--.*?-->"#,
    )
    .unwrap();
    for m in always_ignored_re.find_iter(source) {
        ignored_ranges.push(IgnoredRange {
            start: m.start(),
            end: m.end(),
            rules: Vec::new(),
        });
    }

    let off_patterns = [
        (
            r#"(?is)<!--\s*djlint:off(.*?)\s*-->"#,
            r#"(?is)<!--\s*djlint:on\s*-->"#,
        ),
        (
            r#"(?is)\{#\s*djlint:off(.*?)\s*#\}"#,
            r#"(?is)\{#\s*djlint:on\s*#\}"#,
        ),
        (
            r#"(?is)\{\{!--\s*djlint:off(.*?)\s*--\}\}"#,
            r#"(?is)\{\{!--\s*djlint:on\s*--\}\}"#,
        ),
    ];

    for (off_pat, on_pat) in off_patterns {
        let off_re = Regex::new(off_pat).unwrap();
        let on_re = Regex::new(on_pat).unwrap();

        for off_match in off_re.find_iter(source) {
            let caps = off_re.captures(off_match.as_str()).unwrap();
            let rules_str = caps.get(1).unwrap().as_str();
            let rules: Vec<String> = rules_str
                .split(|c: char| c == ',' || c.is_whitespace())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect();

            let search_start = off_match.end();
            let end = if let Some(on_match) = on_re.find(&source[search_start..]) {
                search_start + on_match.end()
            } else {
                source.len()
            };

            ignored_ranges.push(IgnoredRange {
                start: off_match.start(),
                end,
                rules,
            });
        }
    }

    let is_ignored = |offset: usize, len: usize, code: Option<&str>| -> bool {
        let end = offset + len;
        ignored_ranges.iter().any(|ir| {
            let matches_code = if ir.rules.is_empty() {
                true
            } else if let Some(c) = code {
                ir.rules.iter().any(|r| r == c)
            } else {
                false
            };

            if matches_code {
                if ir.rules.is_empty() {
                    // For general ignores (like comments/script), use the stricter check
                    // that matches djlint's behavior: match must start BEFORE the end of the ignored block.
                    (offset >= ir.start && offset < ir.end) || (end >= ir.start && end <= ir.end)
                } else {
                    // For djlint:off blocks, use overlap
                    offset < ir.end && end > ir.start
                }
            } else {
                false
            }
        })
    };

    // Parity Hack: djlint regex for H030/H031 matches from the VERY FIRST <html> tag.
    // If that tag is inside a comment, the whole match is ignored.
    let mut html_is_ignored = false;
    if let Some(m) = Regex::new(r#"(?i)<html"#).unwrap().find(source) {
        if is_ignored(m.start(), m.len(), None) {
            html_is_ignored = true;
        }
    }

    let masked_source = mask_template_tags(source);

    // Batch 3 Regex
    let extra_blank_lines_pattern = r#"[^\n]{0,10}\n{3,}"#.to_string();
    let extra_blank_lines_re = Regex::new(&extra_blank_lines_pattern).unwrap();

    // Run whole-source regexes (like extra blank lines)
    for cap in extra_blank_lines_re.captures_iter(&masked_source) {
        let mat = cap.get(0).unwrap();
        if is_ignored(mat.start(), mat.len(), Some("H014")) {
            continue;
        }
        let match_str = mat.as_str();
        let line_number = source[..mat.start()].chars().filter(|&c| c == '\n').count() + 1;

        errors.push(LintError {
            code: "H014".to_string(),
            line: line_number,
            column: 0,
            match_str: match_str.replace('\n', "").to_string(),
            message: "Found extra blank lines.".to_string(),
        });
    }

    // Parity Rule H037: Duplicate attribute
    // We simulate djlint's broken regex behavior which can jump across tags
    // due to nested quotes in template tags.
    let attr_start_re = Regex::new(r#"(?i)\s([a-z0-9:-]+)="#).unwrap();
    let lookahead_item_re = Regex::new(
        r#"(?is)^(?:"[^"]*"|'[^']*'|\{\{[\s\S]*?\}\}|\{%[\s\S]*?%\}|#[\s\S]*?#|[^'">{}])"#,
    )
    .unwrap();

    // Batch 1 Rules Regex
    let unquoted_attr_re = Regex::new(r#"(?i)\s+(?:class|id|src|width|height|alt|style|lang|title|href|action|method|checked|required|srcset)=[^"'{>][^\s>]*"#).unwrap();
    let space_around_eq_re =
        Regex::new(r#"(?i)(?:\b|[a-z0-9:@\.-])[a-z0-9:@\.-]*\s+=|=\s+["'{a-z0-9]"#).unwrap();
    let quote_re = Regex::new(r#""[^"]*"|'[^']*'"#).unwrap();
    let js_link_re = Regex::new(r#"(?i)(?:href|action|data-url)=['"]javascript:"#).unwrap();
    let inline_style_re = Regex::new(r#"(?i)\bstyle=["']"#).unwrap();
    let http_link_re = Regex::new(r#"(?i)(?:href|src|action|data-url)=['"]http://"#).unwrap();
    let script_style_type_re =
        Regex::new(r#"(?i)\btype=['"](?:text/css|text/javascript)['"]"#).unwrap();
    let empty_id_class_re = Regex::new(r#"(?i)\b(?:id|class)=['"]['"]"#).unwrap();

    // Batch 2 State
    let mut has_doctype = false;
    let mut has_title = false;
    let mut has_meta_description = false;
    let mut has_meta_keywords = false;
    let mut html_tag_pos: Option<(usize, usize, usize, String)> = None;
    let form_action_ws_re =
        Regex::new(r#"(?i)\baction=(?:\"\s+[^\"]*\s+\"|'\s+[^']*\s+')"#).unwrap();
    let attr_name_re = Regex::new(r#"(?i)\s([a-zA-Z0-9:-]+)="#).unwrap();
    let method_re = Regex::new(r#"(?i)method=['"]([A-Z0-9]+)['"]"#).unwrap();
    let entity_re = Regex::new(r#"&(?:[a-zA-Z0-9]+|#[0-9]+|#x[0-9a-fA-F]+);"#).unwrap();

    // Batch 3 Regex
    let spaceless_tags_re = Regex::new(r#"(?i)\b(?:class|id)=["']\s+\{%|%\}\s+["']"#).unwrap();
    let malformed_tag_re = Regex::new(r#"\{%[^}]*?\}%"#).unwrap();

    for mat in attr_start_re.find_iter(source) {
        if is_ignored(mat.start(), mat.len(), Some("H037")) {
            continue;
        }

        let caps = attr_start_re.captures(mat.as_str()).unwrap();
        let attr_name = caps.get(1).unwrap().as_str().to_lowercase();

        // Simulating djlint's lookahead: (?==(?:\"[^\"]*\"|'[^']*'|{{(?:(?!}}).)*}}|{%(?:(?!%}).)*%}|{#(?:(?!#}).)*#}|[^'\">{}])*[^-:a-z]\1=[^>]*?>)
        let mut curr = mat.end();
        while curr < source.len() {
            let remaining = &source[curr..];

            // Check for duplicate: [^-:a-z]\1=[^>]*?>
            // Note: djlint regex requires it to eventually end with > within the same lookahead match.
            let dup_pattern = format!(r#"(?i)^[^-:a-z]{}="#, regex::escape(&attr_name));
            if let Some(m) = Regex::new(&dup_pattern).unwrap().find(remaining) {
                let after_dup = &remaining[m.end()..];
                if let Some(_tag_end_idx) = after_dup.find('>') {
                    // Check if there are ANY non-ignored '>' between the duplicate and the tag end.
                    // Actually, the djlint regex just takes the first '>' it finds.
                    let line = source[..mat.start()].chars().filter(|&c| c == '\n').count() + 1;
                    errors.push(LintError {
                        code: "H037".to_string(),
                        line,
                        column: 0, // approximate
                        match_str: attr_name.clone(),
                        message: "Duplicate attribute found.".to_string(),
                    });
                    break;
                }
            }

            // Consume one "item" as defined by djlint regex alternatives
            if let Some(m) = lookahead_item_re.find(remaining) {
                curr += m.end();
            } else {
                break; // Hit '>' (which is excluded by lookahead_item_re) or EOF
            }
        }
    }

    let mut i = 0;
    while i < tokens.len() {
        let token = &tokens[i];
        let token_offset = token.offset();
        let token_len = token.raw().len();

        let token_is_ignored = is_ignored(token_offset, token_len, None);

        match token {
            Token::Doctype { .. } if !token_is_ignored => {
                has_doctype = true;
            }
            Token::Tag {
                name,
                raw,
                is_closing,
                is_self_closing,
                line,
                column,
                offset,
            } => {
                let name_lower = name.to_lowercase();
                let raw_lower = raw.to_lowercase();
                let masked_raw = mask_template_tags(raw);

                // Rule H009: Tag names should be lowercase
                if !is_ignored(token_offset, token_len, Some("H009")) {
                    let h009_tags = [
                        "HTML",
                        "BODY",
                        "DIV",
                        "P",
                        "SPAN",
                        "TABLE",
                        "TR",
                        "TD",
                        "TH",
                        "THEAD",
                        "TBODY",
                        "CODE",
                        "UL",
                        "OL",
                        "LI",
                        "H1",
                        "H2",
                        "H3",
                        "H4",
                        "H5",
                        "H6",
                        "A",
                        "DD",
                        "DT",
                        "BLOCKQUOTE",
                        "SELECT",
                        "FORM",
                        "FIELDSET",
                        "OPTGROUP",
                        "LEGEND",
                        "LABEL",
                        "HEADER",
                        "CACHE",
                        "MAIN",
                        "ASIDE",
                        "FOOTER",
                        "SECTION",
                        "NAME",
                        "FIGURE",
                        "FIGCAPTION",
                        "VIDEO",
                        "G",
                        "SVG",
                        "BUTTON",
                        "PATH",
                        "PICTURE",
                        "SCRIPT",
                        "STYLE",
                        "DETAILS",
                        "SUMMARY",
                    ];
                    if h009_tags.contains(name) {
                        errors.push(LintError {
                            code: "H009".to_string(),
                            line: *line,
                            column: *column,
                            match_str: raw.to_string(),
                            message: "Tag names should be lowercase.".to_string(),
                        });
                    }
                }

                if *is_closing {
                    let mut found = false;
                    for j in (0..open_tags.len()).rev() {
                        if open_tags[j].0 == name_lower {
                            open_tags.remove(j);
                            found = true;
                            break;
                        }
                    }
                    if !found && !is_ignored(token_offset, token_len, Some("H025")) {
                        errors.push(LintError {
                            code: "H025".to_string(),
                            line: *line,
                            column: *column,
                            match_str: raw.to_string(),
                            message: "Tag seems to be an orphan.".to_string(),
                        });
                    }

                    if token_is_ignored {
                        i += 1;
                        continue;
                    }

                    // Rule H015: Follow h tags with a line break
                    if name_lower.starts_with('h')
                        && name_lower.len() == 2
                        && name_lower.chars().nth(1).unwrap().is_ascii_digit()
                        && i + 1 < tokens.len()
                        && !is_ignored(token_offset, token_len, Some("H015"))
                    {
                        if let Token::Tag {
                            line: next_line,
                            raw: next_raw,
                            ..
                        } = &tokens[i + 1]
                        {
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
                } else {
                    if name_lower == "html" && html_tag_pos.is_none() {
                        if !token_is_ignored {
                            html_tag_pos = Some((*line, *column, *offset, raw.to_string()));
                            // Rule H007: DOCTYPE before html
                            if !has_doctype
                                && *line == 1
                                && *column == 0
                                && !is_ignored(token_offset, token_len, Some("H007"))
                            {
                                errors.push(LintError {
                                    code: "H007".to_string(),
                                    line: *line,
                                    column: *column,
                                    match_str: raw.to_string(),
                                    message:
                                        "<!DOCTYPE ... > should be present before the html tag."
                                            .to_string(),
                                });
                            }
                        } else {
                            // Parity Hack: if the first <html> tag is ignored,
                            // djlint regex matches it and then ignores the whole rule.
                            // We simulate this by setting a dummy value that prevents further detection
                            // but isn't reported.
                            html_tag_pos = Some((0, 0, 0, "IGNORED".to_string()));
                        }
                    }

                    if !token_is_ignored {
                        if name_lower == "title" {
                            has_title = true;
                        }
                        if name_lower == "meta" {
                            if raw_lower.contains("name=\"description\"")
                                || raw_lower.contains("name='description'")
                            {
                                has_meta_description = true;
                            }
                            if raw_lower.contains("name=\"keywords\"")
                                || raw_lower.contains("name='keywords'")
                            {
                                has_meta_keywords = true;
                            }

                            // Rule H035: Meta tags should be self closing
                            if !raw.ends_with("/>")
                                && !is_ignored(token_offset, token_len, Some("H035"))
                            {
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
                        if is_void_element(&name_lower)
                            && name_lower != "meta"
                            && !raw.ends_with("/>")
                            && !is_ignored(token_offset, token_len, Some("H017"))
                        {
                            errors.push(LintError {
                                code: "H017".to_string(),
                                line: *line,
                                column: *column,
                                match_str: raw.to_string(),
                                message: "Void tags should be self closing.".to_string(),
                            });
                        }

                        // Rule H020: Empty tag pair
                        if !is_self_closing
                            && !is_void_element(&name_lower)
                            && !matches!(name_lower.as_str(), "td" | "li" | "th" | "dt" | "dd")
                            && !raw.contains(' ')
                            && !is_ignored(token_offset, token_len, Some("H020"))
                        {
                            let mut next_tag_idx = i + 1;
                            while next_tag_idx < tokens.len() {
                                match &tokens[next_tag_idx] {
                                    Token::Text { raw, .. } if raw.trim().is_empty() => {
                                        next_tag_idx += 1;
                                    }
                                    Token::Tag {
                                        is_closing: true,
                                        name: next_name,
                                        ..
                                    } => {
                                        if next_name.to_lowercase() == name_lower {
                                            errors.push(LintError {
                                                code: "H020".to_string(),
                                                line: *line,
                                                column: *column,
                                                match_str: raw.to_string(),
                                                message: "Empty tag pair found. Consider removing."
                                                    .to_string(),
                                            });
                                        }
                                        break;
                                    }
                                    _ => break,
                                }
                            }
                        }

                        // Rule H033: Form action whitespace
                        if name_lower == "form"
                            && form_action_ws_re.is_match(raw)
                            && !is_ignored(token_offset, token_len, Some("H033"))
                        {
                            errors.push(LintError {
                                code: "H033".to_string(),
                                line: *line,
                                column: *column,
                                match_str: raw.to_string(),
                                message: "Extra whitespace found in form action.".to_string(),
                            });
                        }

                        // Rule T028: Consider using spaceless tags inside attribute values
                        if let Some(m) = spaceless_tags_re.find(raw) {
                            if !is_ignored(token_offset + m.start(), m.len(), Some("T028")) {
                                errors.push(LintError {
                                    code: "T028".to_string(),
                                    line: *line,
                                    column: *column,
                                    match_str: m.as_str().to_string(),
                                    message: "Consider using spaceless tags inside attribute values. {%- if/for -%}".to_string(),
                                });
                            }
                        }

                        // Rule H010: Attribute names should be lowercase
                        let h010_attrs = [
                            "CLASS", "ID", "SRC", "WIDTH", "HEIGHT", "ALT", "STYLE", "LANG",
                            "TITLE", "MEDIA", "SRCSET",
                        ];
                        for cap in attr_name_re.captures_iter(raw) {
                            let mat = cap.get(0).unwrap();
                            let attr_name = cap.get(1).unwrap().as_str();
                            if h010_attrs.contains(&attr_name)
                                && !is_ignored(token_offset + mat.start(), mat.len(), Some("H010"))
                            {
                                errors.push(LintError {
                                    code: "H010".to_string(),
                                    line: *line,
                                    column: *column,
                                    match_str: attr_name.to_string(),
                                    message: "Attribute names should be lowercase.".to_string(),
                                });
                            }
                        }

                        // Rule H005: lang attribute on html tag
                        if name_lower == "html"
                            && !raw_lower.contains("lang=")
                            && !is_ignored(token_offset, token_len, Some("H005"))
                        {
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
                            if (!raw_lower.contains("height=") || !raw_lower.contains("width="))
                                && !is_ignored(token_offset, token_len, Some("H006"))
                            {
                                errors.push(LintError {
                                    code: "H006".to_string(),
                                    line: *line,
                                    column: *column,
                                    match_str: raw.to_string(),
                                    message: "Img tag should have height and width attributes."
                                        .to_string(),
                                });
                            }
                            if !raw_lower.contains("alt=")
                                && !is_ignored(token_offset, token_len, Some("H013"))
                            {
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
                        if let Some(m) = single_quote_attr_re.find(&masked_raw) {
                            let attr_content = &raw[m.start()..m.end()];
                            // djlint ignores attributes that contain template tags
                            if !attr_content.contains("{{")
                                && !attr_content.contains("{%")
                                && !is_ignored(token_offset + m.start(), m.len(), Some("H008"))
                            {
                                errors.push(LintError {
                                    code: "H008".to_string(),
                                    line: *line,
                                    column: *column,
                                    match_str: raw[..m.end()].to_string(),
                                    message: "Attributes should be double quoted.".to_string(),
                                });
                            }
                        }

                        // Rule H011: Attribute values should be quoted
                        if let Some(m) = unquoted_attr_re.find(&masked_raw) {
                            let attr_content = &raw[m.start()..m.end()];
                            // djlint ignores attributes that contain template tags
                            if !attr_content.contains("{{")
                                && !attr_content.contains("{%")
                                && !is_ignored(token_offset + m.start(), m.len(), Some("H011"))
                            {
                                errors.push(LintError {
                                    code: "H011".to_string(),
                                    line: *line,
                                    column: *column,
                                    match_str: m.as_str().to_string(),
                                    message: "Attribute values should be quoted.".to_string(),
                                });
                            }
                        }

                        // Rule H012: There should be no spaces around attribute =
                        for m in space_around_eq_re.find_iter(&masked_raw) {
                            let match_start = m.start();
                            let match_end = m.end();

                            // Check if this match is inside quotes
                            let mut is_inside_quotes = false;
                            for qm in quote_re.find_iter(&masked_raw) {
                                if match_start >= qm.start() && match_end <= qm.end() {
                                    is_inside_quotes = true;
                                    break;
                                }
                            }

                            if is_inside_quotes {
                                continue;
                            }

                            let attr_content = &raw[m.start()..m.end()];
                            // djlint ignores attributes that contain template tags
                            if !attr_content.contains("{{")
                                && !attr_content.contains("{%")
                                && !is_ignored(token_offset + m.start(), m.len(), Some("H012"))
                            {
                                errors.push(LintError {
                                    code: "H012".to_string(),
                                    line: *line,
                                    column: *column,
                                    match_str: m.as_str().to_string(),
                                    message: "There should be no spaces around attribute =."
                                        .to_string(),
                                });
                            }
                        }

                        // Rule H019: Replace 'javascript:abc()'
                        if let Some(m) = js_link_re.find(raw) {
                            if !is_ignored(token_offset + m.start(), m.len(), Some("H019")) {
                                errors.push(LintError {
                                    code: "H019".to_string(),
                                    line: *line,
                                    column: *column,
                                    match_str: m.as_str().to_string(),
                                    message:
                                        "Replace 'javascript:abc()' with on_ event and real url."
                                            .to_string(),
                                });
                            }
                        }

                        // Rule H021: Inline styles should be avoided
                        if let Some(m) = inline_style_re.find(raw) {
                            // djlint ignores styles that contain template tags
                            if !raw.contains("{{")
                                && !raw.contains("{%")
                                && !is_ignored(token_offset + m.start(), m.len(), Some("H021"))
                            {
                                errors.push(LintError {
                                    code: "H021".to_string(),
                                    line: *line,
                                    column: *column,
                                    match_str: m.as_str().to_string(),
                                    message: "Inline styles should be avoided.".to_string(),
                                });
                            }
                        }

                        // Rule H022: Use HTTPS for external links
                        if let Some(m) = http_link_re.find(raw) {
                            if !is_ignored(token_offset + m.start(), m.len(), Some("H022")) {
                                errors.push(LintError {
                                    code: "H022".to_string(),
                                    line: *line,
                                    column: *column,
                                    match_str: m.as_str().to_string(),
                                    message: "Use HTTPS for external links.".to_string(),
                                });
                            }
                        }

                        // Rule H024: Omit type on scripts and styles
                        if (name_lower == "script" || name_lower == "style")
                            && script_style_type_re.is_match(raw)
                            && !is_ignored(token_offset, token_len, Some("H024"))
                        {
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
                            if !is_ignored(token_offset + m.start(), m.len(), Some("H026")) {
                                errors.push(LintError {
                                    code: "H026".to_string(),
                                    line: *line,
                                    column: *column,
                                    match_str: m.as_str().to_string(),
                                    message: "Empty id and class tags can be removed.".to_string(),
                                });
                            }
                        }

                        // Rule H029: Consider using lowercase form method values
                        if name_lower == "form"
                            && !is_ignored(token_offset, token_len, Some("H029"))
                        {
                            if let Some(caps) = method_re.captures(raw) {
                                let method_val = caps.get(1).unwrap().as_str();
                                if method_val.chars().any(|c| c.is_uppercase()) {
                                    errors.push(LintError {
                                        code: "H029".to_string(),
                                        line: *line,
                                        column: *column,
                                        match_str: caps.get(0).unwrap().as_str().to_string(),
                                        message: "Consider using lowercase form method values."
                                            .to_string(),
                                    });
                                }
                            }
                        }

                        // Rule H036: Avoid use of <br> tags
                        if name_lower == "br" && !is_ignored(token_offset, token_len, Some("H036"))
                        {
                            errors.push(LintError {
                                code: "H036".to_string(),
                                line: *line,
                                column: *column,
                                match_str: raw.to_string(),
                                message: "Avoid use of <br> tags.".to_string(),
                            });
                        }

                        // Rule D004 & J004: Static urls
                        if (name_lower == "link" || name_lower == "script" || name_lower == "img")
                            && (raw.contains("src=\"/static/") || raw.contains("href=\"/static/"))
                        {
                            if !is_ignored(token_offset, token_len, Some("D004")) {
                                errors.push(LintError {
                                    code: "D004".to_string(),
                                    line: *line,
                                    column: *column,
                                    match_str: raw.to_string(),
                                    message: "(Django) Static urls should follow {% static path/to/file %} pattern.".to_string(),
                                });
                            }
                            if !is_ignored(token_offset, token_len, Some("J004")) {
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
                        if (name_lower == "a" || name_lower == "form")
                            && (raw.contains("href=\"/") || raw.contains("action=\"/"))
                            && !raw.contains("href=\"#")
                            && !raw.contains("action=\"#")
                            && !raw.contains("{% url")
                        {
                            if !is_ignored(token_offset, token_len, Some("D018")) {
                                errors.push(LintError {
                                    code: "D018".to_string(),
                                    line: *line,
                                    column: *column,
                                    match_str: raw.to_string(),
                                    message:
                                        "(Django) Internal links should use the {% url ... %} pattern."
                                            .to_string(),
                                });
                            }
                            if !is_ignored(token_offset, token_len, Some("J018")) {
                                errors.push(LintError {
                                    code: "J018".to_string(),
                                    line: *line,
                                    column: *column,
                                    match_str: raw.to_string(),
                                    message:
                                        "(Jinja) Internal links should use the {{ url_for() ... }} pattern."
                                            .to_string(),
                                });
                            }
                        }
                    }

                    if !is_self_closing && !is_void_element(&name_lower) {
                        open_tags.push((name_lower, *line, *column, *offset));
                    }
                }
            }
            Token::DjangoVar {
                raw,
                line,
                column,
                offset: _,
            }
            | Token::DjangoBlock {
                raw,
                line,
                column,
                offset: _,
            } if !token_is_ignored => {
                let is_var = matches!(token, Token::DjangoVar { .. });
                let (open_tag, close_tag) = if is_var { ("{{", "}}") } else { ("{%", "%}") };

                if (!raw.starts_with(&format!("{} ", open_tag))
                    || !raw.ends_with(&format!(" {}", close_tag)))
                    && !is_ignored(token_offset, token_len, Some("T001"))
                {
                    errors.push(LintError {
                        code: "T001".to_string(),
                        line: *line,
                        column: *column,
                        match_str: raw.to_string(),
                        message: "Variables should be wrapped in a whitespace.".to_string(),
                    });
                }

                if !is_var {
                    if raw.contains('\'')
                        && (raw.contains("extends")
                            || raw.contains("include")
                            || raw.contains("with")
                            || raw.contains("trans")
                            || raw.contains("now"))
                        && !is_ignored(token_offset, token_len, Some("T002"))
                    {
                        errors.push(LintError {
                            code: "T002".to_string(),
                            line: *line,
                            column: *column,
                            match_str: raw.to_string(),
                            message: "Double quotes should be used in tags.".to_string(),
                        });
                    }

                    let inner = raw.trim_start_matches("{%").trim_end_matches("%}").trim();
                    if inner == "endblock" && !is_ignored(token_offset, token_len, Some("T003")) {
                        errors.push(LintError {
                            code: "T003".to_string(),
                            line: *line,
                            column: *column,
                            match_str: raw.to_string(),
                            message: "Endblock should have name. Ex: {% endblock body %}."
                                .to_string(),
                        });
                    }
                }

                // Rule T027: Unclosed string
                // Very simple heuristic: count single and double quotes
                let single_quotes = raw.chars().filter(|&c| c == '\'').count();
                let double_quotes = raw.chars().filter(|&c| c == '"').count();
                if (single_quotes % 2 != 0 || double_quotes % 2 != 0)
                    && !is_ignored(token_offset, token_len, Some("T027"))
                {
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
                    if !is_ignored(token_offset + m.start(), m.len(), Some("T034")) {
                        errors.push(LintError {
                            code: "T034".to_string(),
                            line: *line,
                            column: *column,
                            match_str: m.as_str().to_string(),
                            message: "Did you intend to use {% ... %} instead of {% ... }%?"
                                .to_string(),
                        });
                    }
                }
            }
            Token::Text {
                raw,
                line,
                column,
                offset: _,
            } if !token_is_ignored => {
                // Rule H023: Do not use entity references
                if let Some(m) = entity_re.find(raw) {
                    let entity = m.as_str();
                    // djlint allows some common ones like &nbsp;, &lt;, &gt;, &amp;, &quot;, &ensp;, &emsp;, &thinsp;, &shy;
                    if !matches!(
                        entity,
                        "&nbsp;"
                            | "&lt;"
                            | "&gt;"
                            | "&amp;"
                            | "&quot;"
                            | "&ensp;"
                            | "&emsp;"
                            | "&thinsp;"
                            | "&shy;"
                    ) && !is_ignored(token_offset + m.start(), m.len(), Some("H023"))
                    {
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
    if let Some((line, column, offset, match_str)) = html_tag_pos {
        let raw_len = match_str.len();
        if match_str != "IGNORED" && !html_is_ignored {
            if !has_title && !is_ignored(offset, raw_len, Some("H016")) {
                errors.push(LintError {
                    code: "H016".to_string(),
                    line,
                    column,
                    match_str: match_str.clone(),
                    message: "Missing title tag in html.".to_string(),
                });
            }
            if !has_meta_description
                && !match_str.contains("[endif]")
                && !is_ignored(offset, raw_len, Some("H030"))
            {
                errors.push(LintError {
                    code: "H030".to_string(),
                    line,
                    column,
                    match_str: match_str.clone(),
                    message: "Consider adding a meta description.".to_string(),
                });
            }
            if !has_meta_keywords
                && !match_str.contains("[endif]")
                && !is_ignored(offset, raw_len, Some("H031"))
            {
                errors.push(LintError {
                    code: "H031".to_string(),
                    line,
                    column,
                    match_str: match_str.clone(),
                    message: "Consider adding meta keywords.".to_string(),
                });
            }
        }
    }

    // After all tokens, if any open_tags left, they are orphans
    for (tag_name, line, column, offset) in open_tags {
        if !is_ignored(offset, tag_name.len() + 2, Some("H025")) {
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

    // Filter by profile
    let excluded_prefixes = match config.profile.to_lowercase().as_str() {
        "all" => vec![],
        "html" => vec!["D", "J", "T", "N", "M"],
        "django" => vec!["J", "N", "M"],
        "jinja" => vec!["D", "N", "M"],
        "nunjucks" => vec!["D", "J", "M"],
        "handlebars" => vec!["D", "J", "N"],
        "golang" => vec!["D", "J", "N", "M"],
        "angular" => vec!["D", "J", "H012", "H026", "H028"],
        _ => vec![],
    };

    let default_false_rules = ["H017", "H035", "H036"];

    // Filter ignored rules and profile exclusions
    errors
        .into_iter()
        .filter(|e| {
            let is_default_false = default_false_rules.contains(&e.code.as_str());
            let is_included = config.include.contains(&e.code);
            let is_ignored = config.ignore.contains(&e.code);
            let is_profile_excluded = excluded_prefixes
                .iter()
                .any(|prefix| e.code.starts_with(prefix));

            !is_ignored && !is_profile_excluded && (!is_default_false || is_included)
        })
        .collect()
}

fn mask_template_tags(raw: &str) -> String {
    let django_block_re = Regex::new(r#"\{%[\s\S]*?%\}"#).unwrap();
    let django_var_re = Regex::new(r#"\{\{[\s\S]*?\}\}"#).unwrap();

    let mut masked = raw.to_string();

    for m in django_block_re.find_iter(raw) {
        let start = m.start();
        let end = m.end();
        masked.replace_range(start..end, &"x".repeat(end - start));
    }

    let current_masked = masked.clone();
    for m in django_var_re.find_iter(&current_masked) {
        let start = m.start();
        let end = m.end();
        masked.replace_range(start..end, &"x".repeat(end - start));
    }

    masked
}

fn is_void_element(name: &str) -> bool {
    matches!(
        name,
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
