use djlintr::{config::Config, format};
use rstest::rstest;

#[rstest]
#[case("", "")]
#[case("<!--hello world-->", "<!--hello world-->\n")]
#[case(
    "<!doctype html>\n<html>\n<head></head>\n<body></body>\n</html>\n",
    "<!DOCTYPE html>\n<html>\n    <head></head>\n    <body></body>\n</html>\n"
)]
fn test_formatter_basics(#[case] source: &str, #[case] expected: &str) {
    let config = Config::default();
    let output = format(&config, source);
    assert_eq!(output, expected);
}
