use djlintr::{config::Config, lint, linter::LintError};
use rstest::rstest;

#[rstest]
#[case(
    "<img alt=\"test\"/>",
    vec![
        LintError {
            code: "H006".to_string(),
            line: 1,
            column: 0,
            match_str: "<img alt=\"test\"/>".to_string(),
            message: "Img tag should have height and width attributes.".to_string(),
        }
    ]
)]
#[case(
    "<img>",
    vec![
        LintError {
            code: "H006".to_string(),
            line: 1,
            column: 0,
            match_str: "<img>".to_string(),
            message: "Img tag should have height and width attributes.".to_string(),
        },
        LintError {
            code: "H013".to_string(),
            line: 1,
            column: 0,
            match_str: "<img>".to_string(),
            message: "Img tag should have an alt attribute.".to_string(),
        }
    ]
)]
fn test_image_rules(#[case] source: &str, #[case] mut expected: Vec<LintError>) {
    let config = Config::default();
    let mut output = lint(&config, source);
    output.retain(|e| e.code != "H017");
    expected.retain(|e| e.code != "H017");
    assert_eq!(output, expected);
}
