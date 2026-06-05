use djlintr::{config::Config, formatter::format};

// Regression test for Cotton-style self-closing component tags inside a div.
// A `{% component ... / %}` tag must not be treated as a block opener;
// the parent div should collapse its content onto a single line.
#[test]
fn test_self_closing_component_inside_div_collapses() {
    let input = "<div class=\"flex-grow\">\n    {% component \"adminlogs\" logs=object.installationprworkflowlog_set.all / %}\n</div>\n";
    let expected = "<div class=\"flex-grow\">{% component \"adminlogs\" logs=object.installationprworkflowlog_set.all / %}</div>\n";

    let config = Config {
        custom_blocks: vec!["component".to_string(), "endcomponent".to_string()],
        ..Config::default()
    };

    let output = format(&config, input);
    assert_eq!(output, expected);
}
