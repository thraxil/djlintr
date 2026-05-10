use djlintr::{config::Config, lint, linter::LintError};
use rstest::rstest;

#[rstest]
#[case(
    "\n\n\n\n<div>",
    vec![
        LintError {
            code: "H014".to_string(),
            line: 1,
            column: 0,
            match_str: "\n\n\n\n".to_string(),
            message: "Found extra blank lines.".to_string(),
        }
    ]
)]
#[case(
    "{% blah 'asdf %}",
    vec![
        LintError {
            code: "T027".to_string(),
            line: 1,
            column: 0,
            match_str: "{% blah 'asdf %}".to_string(),
            message: "Unclosed string found in template syntax.".to_string(),
        }
    ]
)]
#[case(
    "<a class=\"{% if x %}\">",
    vec![
        LintError {
            code: "T028".to_string(),
            line: 1,
            column: 0,
            match_str: "class=\"{% if x %}\"".to_string(),
            message: "Consider using spaceless tags inside attribute values. {%- if/for -%}".to_string(),
        }
    ]
)]
#[case(
    "{% block main }% content {% endblock %}",
    vec![
        LintError {
            code: "T034".to_string(),
            line: 1,
            column: 0,
            match_str: "{% block main }%".to_string(),
            message: "Did you intend to use {% ... %} instead of {% ... }%?".to_string(),
        }
    ]
)]
#[case(
    "<img src=\"/static/foo.png\">",
    vec![
        // We know D004 triggers here, so we expect it (or we test it via J004 later)
        LintError {
            code: "D004".to_string(),
            line: 1,
            column: 0,
            match_str: "<img src=\"/static/foo.png\">".to_string(),
            message: "(Django) Static urls should follow {% static path/to/file %} pattern.".to_string(),
        },
        LintError {
            code: "J004".to_string(),
            line: 1,
            column: 0,
            match_str: "<img src=\"/static/foo.png\">".to_string(),
            message: "(Jinja) Static urls should follow {{ url_for('static'..) }} pattern.".to_string(),
        }
    ]
)]
#[case(
    "<a href=\"/internal\">Link</a>",
    vec![
        LintError {
            code: "D018".to_string(),
            line: 1,
            column: 0,
            match_str: "<a href=\"/internal\">".to_string(),
            message: "(Django) Internal links should use the {% url ... %} pattern.".to_string(),
        },
        LintError {
            code: "J018".to_string(),
            line: 1,
            column: 0,
            match_str: "<a href=\"/internal\">".to_string(),
            message: "(Jinja) Internal links should use the {{ url_for() ... }} pattern.".to_string(),
        }
    ]
)]
fn test_batch_3_rules(#[case] source: &str, #[case] mut expected: Vec<LintError>) {
    let config = Config::default();
    let mut output = lint(&config, source);

    output.retain(|e| {
        [
            "H014", "T027", "T028", "T034", "J004", "J018", "D004", "D018",
        ]
        .contains(&e.code.as_str())
    });
    expected.retain(|e| {
        [
            "H014", "T027", "T028", "T034", "J004", "J018", "D004", "D018",
        ]
        .contains(&e.code.as_str())
    });

    assert_eq!(output, expected);
}
