use djlintr::{config::Config, lint, linter::LintError};
use rstest::rstest;

#[rstest]
#[case(
    "{{test }}\n{% test%}<a>",
    vec![
        LintError {
            code: "T001".to_string(),
            line: 1,
            column: 0,
            match_str: "{{test }}".to_string(),
            message: "Variables should be wrapped in a whitespace.".to_string(),
        },
        LintError {
            code: "T001".to_string(),
            line: 2,
            column: 0,
            match_str: "{% test%}".to_string(),
            message: "Variables should be wrapped in a whitespace.".to_string(),
        },
        LintError {
            code: "H025".to_string(),
            line: 2,
            column: 9,
            match_str: "<a>".to_string(),
            message: "Tag seems to be an orphan.".to_string(),
        }
    ]
)]
#[case(
    "{% extends 'this' %}",
    vec![
        LintError {
            code: "T002".to_string(),
            line: 1,
            column: 0,
            match_str: "{% extends 'this' %}".to_string(),
            message: "Double quotes should be used in tags.".to_string(),
        }
    ]
)]
#[case(
    "{% endblock %}",
    vec![
        LintError {
            code: "T003".to_string(),
            line: 1,
            column: 0,
            match_str: "{% endblock %}".to_string(),
            message: "Endblock should have name. Ex: {% endblock body %}.".to_string(),
        }
    ]
)]
#[case(
    "<link src=\"/static/there\">",
    vec![
        LintError {
            code: "D004".to_string(),
            line: 1,
            column: 0,
            match_str: "<link src=\"/static/there\">".to_string(),
            message: "(Django) Static urls should follow {% static path/to/file %} pattern.".to_string(),
        }
    ]
)]
#[case(
    "<a href=\"/Collections?handler=RemoveAgreement&id=@a.Id\">",
    vec![
        LintError {
            code: "D018".to_string(),
            line: 1,
            column: 0,
            match_str: "<a href=\"/Collections?handler=RemoveAgreement&id=@a.Id\">".to_string(),
            message: "(Django) Internal links should use the {% url ... %} pattern.".to_string(),
        }
    ]
)]
#[case(
    "{% block title %}- User Create{% endblock %}",
    vec![
        LintError {
            code: "T003".to_string(),
            line: 1,
            column: 30,
            match_str: "{% endblock %}".to_string(),
            message: "Endblock should have name. Ex: {% endblock body %}.".to_string(),
        }
    ]
)]
#[case(
    "{% block title %}\n- User Create\n{% endblock %}",
    vec![
        LintError {
            code: "T003".to_string(),
            line: 3,
            column: 0,
            match_str: "{% endblock %}".to_string(),
            message: "Endblock should have name. Ex: {% endblock body %}.".to_string(),
        }
    ]
)]
#[case(
    "<a class=\"{% if foo == 'bar' %}{% endif %}\">Foo</a>",
    vec![]
)]
#[case(
    "<a class=\" {% if foo == 'bar' %} \">Foo</a>",
    vec![
        LintError {
            code: "T028".to_string(),
            line: 1,
            column: 0,
            match_str: "class=\" {%".to_string(),
            message: "Consider using spaceless tags inside attribute values. {%- if/for -%}".to_string(),
        }
    ]
)]
#[case(
    "<a href=\"?foo={{ bar }}\">foo</a>",
    vec![]
)]
#[case(
    "{% block foo %}\n    Bar\n{% endblock %}",
    vec![
        LintError {
            code: "T003".to_string(),
            line: 3,
            column: 0,
            match_str: "{% endblock %}".to_string(),
            message: "Endblock should have name. Ex: {% endblock body %}.".to_string(),
        }
    ]
)]
fn test_django_linter(#[case] source: &str, #[case] mut expected: Vec<LintError>) {
    let mut config = Config::default();
    config.profile = "all".to_string();
    let mut output = lint(&config, source);

    let ignore_codes = ["H025", "H017", "J004", "J018"];
    output.retain(|e| !ignore_codes.contains(&e.code.as_str()));
    expected.retain(|e| !ignore_codes.contains(&e.code.as_str()));

    assert_eq!(output, expected);
}
