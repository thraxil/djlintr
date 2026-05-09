use djlintr::{config::Config, lint, linter::LintError};
use rstest::rstest;

#[rstest]
#[case(
    "<DIV>",
    vec![
        LintError {
            code: "H009".to_string(),
            line: 1,
            column: 0,
            match_str: "<DIV>".to_string(),
            message: "Tag names should be lowercase.".to_string(),
        },
        LintError {
            code: "H025".to_string(),
            line: 1,
            column: 0,
            match_str: "<div>".to_string(), // match_str might need adjustment in linter
            message: "Tag seems to be an orphan.".to_string(),
        }
    ]
)]
#[case(
    "<div CLASS=\"test\">",
    vec![
        LintError {
            code: "H010".to_string(),
            line: 1,
            column: 0,
            match_str: "CLASS=".to_string(),
            message: "Attribute names should be lowercase.".to_string(),
        }
    ]
)]
fn test_case_rules(#[case] source: &str, #[case] expected: Vec<LintError>) {
    let config = Config::default();
    let output = lint(&config, source);
    // Note: H025 match_str in expected might need careful handling
    assert_eq!(output, expected);
}
