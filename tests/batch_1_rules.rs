use djlintr::{config::Config, lint, linter::LintError};
use rstest::rstest;

#[rstest]
#[case(
    "<div class=test>",
    vec![
        LintError {
            code: "H011".to_string(),
            line: 1,
            column: 0,
            match_str: " class=test".to_string(),
            message: "Attribute values should be quoted.".to_string(),
        }
    ]
)]
#[case(
    "<div class = \"test\">",
    vec![
        LintError {
            code: "H012".to_string(),
            line: 1,
            column: 0,
            match_str: "class =".to_string(),
            message: "There should be no spaces around attribute =.".to_string(),
        }
    ]
)]
#[case(
    "<a href=\"javascript:void(0)\">",
    vec![
        LintError {
            code: "H019".to_string(),
            line: 1,
            column: 0,
            match_str: "href=\"javascript:".to_string(),
            message: "Replace 'javascript:abc()' with on_ event and real url.".to_string(),
        }
    ]
)]
#[case(
    "<div style=\"color: red;\">",
    vec![
        LintError {
            code: "H021".to_string(),
            line: 1,
            column: 0,
            match_str: "style=\"".to_string(),
            message: "Inline styles should be avoided.".to_string(),
        }
    ]
)]
#[case(
    "<a href=\"http://example.com\">",
    vec![
        LintError {
            code: "H022".to_string(),
            line: 1,
            column: 0,
            match_str: "href=\"http://".to_string(),
            message: "Use HTTPS for external links.".to_string(),
        }
    ]
)]
#[case(
    "<div>&#169;</div>",
    vec![
        LintError {
            code: "H023".to_string(),
            line: 1,
            column: 5,
            match_str: "&#169;".to_string(),
            message: "Do not use entity references.".to_string(),
        }
    ]
)]
#[case(
    "<script type=\"text/javascript\"></script>",
    vec![
        LintError {
            code: "H024".to_string(),
            line: 1,
            column: 0,
            match_str: "<script type=\"text/javascript\">".to_string(),
            message: "Omit type on scripts and styles.".to_string(),
        }
    ]
)]
#[case(
    "<div id=\"\">",
    vec![
        LintError {
            code: "H026".to_string(),
            line: 1,
            column: 0,
            match_str: "id=\"\"".to_string(),
            message: "Empty id and class tags can be removed.".to_string(),
        }
    ]
)]
#[case(
    "<form method=\"POST\">",
    vec![
        LintError {
            code: "H029".to_string(),
            line: 1,
            column: 0,
            match_str: "method=\"POST\"".to_string(),
            message: "Consider using lowercase form method values.".to_string(),
        }
    ]
)]
#[case(
    "<br>",
    vec![
        LintError {
            code: "H036".to_string(),
            line: 1,
            column: 0,
            match_str: "<br>".to_string(),
            message: "Avoid use of <br> tags.".to_string(),
        }
    ]
)]
fn test_batch_1_rules(#[case] source: &str, #[case] mut expected: Vec<LintError>) {
    let config = Config::default();
    let mut output = lint(&config, source);
    // filter out H025, H020, and H017 for these isolated tests to make them cleaner
    output.retain(|e| !["H025", "H020", "H017"].contains(&e.code.as_str()));
    expected.retain(|e| !["H025", "H020", "H017"].contains(&e.code.as_str()));
    assert_eq!(output, expected);
}
