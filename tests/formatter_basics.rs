use djlintr::{config::Config, format};
use rstest::rstest;

#[rstest]
#[case("", "")]
#[case("<!--hello world-->", "<!--hello world-->\n")]
#[case(
    "<!doctype html>\n<html>\n<head></head>\n<body></body>\n</html>\n",
    "<!DOCTYPE html>\n<html>\n    <head></head>\n    <body></body>\n</html>\n"
)]
#[case(
    "<div>\n{% include 'test.html' %}\n</div>",
    "<div>{% include 'test.html' %}</div>\n"
)]
#[case(
    "<_tag label=\"a\" label=\"b\"></_tag>",
    "<_tag label=\"a\" label=\"b\"></_tag>\n"
)]
#[case(
    "{# djlint:off #}\n<div   class='foo'  >\n{# djlint:on #}",
    "{# djlint:off #}\n<div   class='foo'  >\n{# djlint:on #}\n"
)]
#[case(
    "<div>\n    line1\n    line2\n</div>",
    "<div>\n    line1\n    line2\n</div>\n"
)]
fn test_formatter_basics(#[case] source: &str, #[case] expected: &str) {
    let config = Config::default();
    let output = format(&config, source);
    assert_eq!(output, expected);
}
