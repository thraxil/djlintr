use djlintr::{config::Config, formatter::format};

// {% comment %}...{% endcomment %} blocks are treated as verbatim by Python djlint:
// content is preserved as-is without indenting, reformatting, or collapsing.

#[test]
fn test_comment_block_not_collapsed() {
    // Multi-line comment is never condensed to one line.
    let input = "{% comment %}\ntest\n{% endcomment %}\n";
    let expected = "{% comment %}\ntest\n{% endcomment %}\n";
    assert_eq!(format(&Config::default(), input), expected);
}

#[test]
fn test_comment_block_content_not_indented() {
    // Content inside {% comment %} is not reformatted.
    let input = "{% comment %}\nline 1\nline 2\n{% endcomment %}\n";
    let expected = "{% comment %}\nline 1\nline 2\n{% endcomment %}\n";
    assert_eq!(format(&Config::default(), input), expected);
}

#[test]
fn test_comment_block_attributes_not_wrapped() {
    // HTML inside {% comment %} is preserved verbatim (no attribute wrapping).
    let input = "{% comment %}\n    <div attr1=\"very-long-attribute-value-1234567890-1234567890-1234567890\" attr2=\"very-long-attribute-value-1234567890-1234567890-1234567890\">\n    </div>\n{% endcomment %}\n";
    let expected = input;
    assert_eq!(format(&Config::default(), input), expected);
}

#[test]
fn test_inline_comment_stays_inline() {
    // Single-line comment with no inner newline stays on one line.
    let input = "{% comment %}SVG icon info{% endcomment %}\n";
    let expected = "{% comment %}SVG icon info{% endcomment %}\n";
    assert_eq!(format(&Config::default(), input), expected);
}

#[test]
fn test_comment_endtag_indented_to_opener_level() {
    // {% endcomment %} is placed at the same indent level as {% comment %},
    // not at the level of the verbatim content's own trailing whitespace.
    let input = "<div>\n    <a href=\"x\">\n        <div>\n            {% comment %}\n                text\n            {% endcomment %}\n        </div>\n    </a>\n</div>\n";
    let output = format(&Config::default(), input);
    assert!(
        output.contains("            {% endcomment %}"),
        "endcomment should be at 12-space indent, got:\n{}",
        output
    );
}
