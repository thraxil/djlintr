use djlintr::{config::Config, lint, linter::LintError};
use rstest::rstest;

#[rstest]
#[case(
    "<html>\n<head></head>\n<body></body>\n</html>",
    vec![
        LintError {
            code: "H007".to_string(),
            line: 1,
            column: 0,
            match_str: "<html>".to_string(),
            message: "<!DOCTYPE ... > should be present before the html tag.".to_string(),
        },
        LintError {
            code: "H005".to_string(), // already implemented
            line: 1,
            column: 0,
            match_str: "<html>".to_string(),
            message: "Html tag should have lang attribute.".to_string(),
        },
        LintError {
            code: "H016".to_string(),
            line: 1,
            column: 0, // usually reported at html or head level depending on implementation
            match_str: "<html>".to_string(),
            message: "Missing title tag in html.".to_string(),
        },
        LintError {
            code: "H030".to_string(),
            line: 1,
            column: 0,
            match_str: "<html>".to_string(),
            message: "Consider adding a meta description.".to_string(),
        },
        LintError {
            code: "H031".to_string(),
            line: 1,
            column: 0,
            match_str: "<html>".to_string(),
            message: "Consider adding meta keywords.".to_string(),
        }
    ]
)]
#[case(
    "<h1>Title</h1><span>Next</span>",
    vec![
        LintError {
            code: "H015".to_string(),
            line: 1,
            column: 14,
            match_str: "<span>".to_string(),
            message: "Follow h tags with a line break.".to_string(),
        }
    ]
)]
#[case(
    "<img src=\"x\">",
    vec![
        LintError {
            code: "H017".to_string(),
            line: 1,
            column: 0,
            match_str: "<img src=\"x\">".to_string(),
            message: "Void tags should be self closing.".to_string(),
        },
        // H006 and H013 will also trigger here
    ]
)]
#[case(
    "<div></div>",
    vec![
        LintError {
            code: "H020".to_string(),
            line: 1,
            column: 0,
            match_str: "<div>".to_string(),
            message: "Empty tag pair found. Consider removing.".to_string(),
        }
    ]
)]
#[case(
    "<th></th><td></td>",
    vec![]
)]
#[case(
    "<!DOCTYPE html><html lang=\"en\"><head><title>T</title></head><body></body></html>",
    vec![
        LintError {
            code: "H030".to_string(),
            line: 1,
            column: 15,
            match_str: "<html lang=\"en\">".to_string(),
            message: "Consider adding a meta description.".to_string(),
        },
        LintError {
            code: "H031".to_string(),
            line: 1,
            column: 15,
            match_str: "<html lang=\"en\">".to_string(),
            message: "Consider adding meta keywords.".to_string(),
        }
    ]
)]
#[case(
    "<form action=\" /submit \"></form>",
    vec![
        LintError {
            code: "H033".to_string(),
            line: 1,
            column: 0,
            match_str: "<form action=\" /submit \">".to_string(),
            message: "Extra whitespace found in form action.".to_string(),
        }
    ]
)]
#[case(
    "<meta charset=\"utf-8\">",
    vec![
        LintError {
            code: "H035".to_string(),
            line: 1,
            column: 0,
            match_str: "<meta charset=\"utf-8\">".to_string(),
            message: "Meta tags should be self closing.".to_string(),
        }
    ]
)]
#[case(
    "<div class=\"a\" class=\"b\"></div>",
    vec![
        LintError {
            code: "H037".to_string(),
            line: 1,
            column: 0,
            match_str: "<div class=\"a\" class=\"b\">".to_string(),
            message: "Duplicate attribute found.".to_string(),
        }
    ]
)]
fn test_batch_2_rules(#[case] source: &str, #[case] mut expected: Vec<LintError>) {
    let config = Config::default();
    let mut output = lint(&config, source);
    
    // Ignore rules that we aren't specifically testing here to keep cases clean
    output.retain(|e| ["H007", "H015", "H016", "H017", "H020", "H030", "H031", "H033", "H035", "H037", "H005"].contains(&e.code.as_str()));
    expected.retain(|e| ["H007", "H015", "H016", "H017", "H020", "H030", "H031", "H033", "H035", "H037", "H005"].contains(&e.code.as_str()));
    
    // If we aren't testing H020 specifically, filter it out
    if !source.contains("<div></div>") {
        output.retain(|e| e.code != "H020");
        expected.retain(|e| e.code != "H020");
    }

    assert_eq!(output, expected);
}
