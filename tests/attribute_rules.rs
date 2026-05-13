use djlintr::{config::Config, lint, linter::LintError};
use rstest::rstest;

#[rstest]
#[case(
    "<div class='test'>",
    vec![
        LintError {
            code: "H008".to_string(),
            line: 1,
            column: 0,
            match_str: "<div class='test'".to_string(),
            message: "Attributes should be double quoted.".to_string(),
        }
    ]
)]
#[case(
    "<div class=\"test\">",
    vec![]
)]
fn test_attribute_rules(#[case] source: &str, #[case] expected: Vec<LintError>) {
    let config = Config::default();
    let mut output = lint(&config, source);
    output.retain(|e| e.code != "H025");
    assert_eq!(output, expected);
}
