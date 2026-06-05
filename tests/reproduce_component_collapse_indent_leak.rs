use djlintr::{config::Config, formatter::format};

// When a div collapses its only child (a self-closing custom block tag), Python
// djlint's two-phase expand-then-condense approach still tracks the +1 indent
// from the block opener during the expansion phase.  That increment persists
// after condensation, so subsequent siblings appear one level deeper.

#[test]
fn test_collapsed_component_leaks_indent_to_next_sibling() {
    let input = "<div>\n    {% component \"a\" / %}\n</div>\n<p>b</p>\n";
    let expected = "<div>{% component \"a\" / %}</div>\n    <p>b</p>\n";

    let config = Config {
        custom_blocks: vec!["component".to_string(), "endcomponent".to_string()],
        ..Config::default()
    };

    let output = format(&config, input);
    assert_eq!(output, expected);
}

#[test]
fn test_collapsed_component_leaks_indent_to_outer_closing_tag() {
    // Nested case: the outer </div> is indented because the inner div's
    // collapsed component leaked +1 onto the indent level.
    let input = "<div>\n    <div>{% component \"adminlogs\" / %}</div>\n</div>\n";
    let expected = "<div>\n    <div>{% component \"adminlogs\" / %}</div>\n    </div>\n";

    let config = Config {
        custom_blocks: vec!["component".to_string(), "endcomponent".to_string()],
        ..Config::default()
    };

    let output = format(&config, input);
    assert_eq!(output, expected);
}
