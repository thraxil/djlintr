use djlintr::{config::Config, lint, linter::LintError};
use rstest::rstest;

#[rstest]
#[case(
    "<!DOCTYPE html>\n<html>",
    vec![
        LintError {
            code: "H005".to_string(),
            line: 2,
            column: 0,
            match_str: "<html>".to_string(),
            message: "Html tag should have lang attribute.".to_string(),
        },
        LintError {
            code: "H025".to_string(),
            line: 2,
            column: 0,
            match_str: "<html>".to_string(),
            message: "Tag seems to be an orphan.".to_string(),
        }
    ]
)]
fn test_linter_h005(#[case] source: &str, #[case] mut expected: Vec<LintError>) {
    let config = Config::default();
    let mut output = lint(&config, source);
    output.retain(|e| !["H016", "H030", "H031", "H007"].contains(&e.code.as_str()));
    expected.retain(|e| !["H016", "H030", "H031", "H007"].contains(&e.code.as_str()));
    assert_eq!(output, expected);
}
