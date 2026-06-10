//! Token-tree navigation and inline rendering: locating a tag's children
//! and matching close, grouping them into logical elements, and rendering
//! a range of tokens onto a single line for collapse decisions.
use super::*;

pub(crate) fn get_children_info(index: usize, tokens: &[Token]) -> (Vec<usize>, Option<usize>) {
    let mut children = Vec::new();
    let token = &tokens[index];
    match token {
        Token::Tag { name, .. } => {
            let mut j = index + 1;
            let mut depth = 1;
            while j < tokens.len() {
                // When a template block opens AND closes on the same source
                // line (e.g. `{% if %}..{% endif %}` all on one line), djlint's
                // line-based indenter ignores any HTML opening/closing tags
                // inside that block for indentation purposes.  Mirror this by
                // skipping to the template closer so unbalanced HTML inside
                // the block does not affect our depth counter.
                if let Token::DjangoBlock { raw, .. } = &tokens[j] {
                    let tag_name = get_django_tag_name(raw).unwrap_or("");
                    let is_potential_opener = !tag_name.starts_with("end")
                        && !matches!(tag_name, "else" | "elif" | "empty" | "");
                    if is_potential_opener {
                        let opener_line = tokens[j].line();
                        let mut k = j + 1;
                        let mut block_depth: usize = 1;
                        while k < tokens.len() && tokens[k].line() == opener_line {
                            if let Token::DjangoBlock { raw: kr, .. } = &tokens[k] {
                                let kn = get_django_tag_name(kr).unwrap_or("");
                                if kn == tag_name {
                                    block_depth += 1;
                                } else if kn.starts_with("end") && &kn[3..] == tag_name {
                                    block_depth -= 1;
                                    if block_depth == 0 {
                                        // Found the matching closer on the
                                        // same line — skip to it.
                                        j = k;
                                        break;
                                    }
                                }
                            }
                            k += 1;
                        }
                        // If block_depth > 0, no same-line closer was found;
                        // fall through to normal handling below.
                    }
                }

                match &tokens[j] {
                    Token::Tag {
                        name: n,
                        is_closing: false,
                        is_self_closing: false,
                        ..
                    } if n.eq_ignore_ascii_case(name) => {
                        // Push the nested same-name opener as a child BEFORE
                        // incrementing depth so it is visible to
                        // get_logical_elements (and format_range_inlined can
                        // recurse into it correctly).
                        if depth == 1 {
                            children.push(j);
                        }
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

pub(crate) fn get_logical_elements(
    children: &[usize],
    tokens: &[Token],
) -> Vec<std::ops::Range<usize>> {
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

pub(crate) fn is_tag_range_inlinable(
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
            let formatted = format_tag(
                name,
                raw,
                *is_self_closing,
                0,
                0,
                config,
                is_ignored_block,
                false,
            );
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

pub(crate) fn format_range_inlined(
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
                                false,
                            ));
                            let sub_elements = get_logical_elements(&children, tokens);
                            let inner_content = format_range_inlined_joined(
                                &sub_elements,
                                tokens,
                                indent_level,
                                config,
                            );
                            result.push_str(inner_content.trim_end());
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
                                false,
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
                            false,
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

pub(crate) fn format_range_inlined_joined(
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
