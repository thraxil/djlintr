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
#[case(
    "{% block title %}\n    Internal Server Error\n{% endblock %}",
    "{% block title %}Internal Server Error{% endblock %}\n"
)]
#[case(
    "{% for deck in decks %}<option>{{ deck.name }}</option>{% endfor %}",
    "{% for deck in decks %}<option>{{ deck.name }}</option>{% endfor %}\n"
)]
#[case(
    "{% block title %}<h1 class=\"foo\">Internal Server Error</h1>{% endblock %}",
    "{% block title %}<h1 class=\"foo\">Internal Server Error</h1>{% endblock %}\n"
)]
#[case("<p>\n  line1\n  line2\n</p>", "<p>\n    line1\n    line2\n</p>\n")]
#[case(
    "{% block content %}\nAn activation email has been sent.  Please check your email and click on the link to activate your account.\n{% endblock %}",
    "{% block content %}\n    An activation email has been sent.  Please check your email and click on the link to activate your account.\n{% endblock %}\n"
)]
#[case(
    "<div style=\"color: red; background: blue; border: 1px solid black; padding: 10px; margin: 10px;\"></div>",
    "<div style=\"color: red;\n            background: blue;\n            border: 1px solid black;\n            padding: 10px;\n            margin: 10px\"></div>\n"
)]
#[case(
    "<div id=\"main\" role=\"main\">\n\n</div>",
    "<div id=\"main\" role=\"main\"></div>\n"
)]
#[case(
    "<link rel=\"stylesheet\" media=\"handheld\" href=\"{\" static \"css/handheld.css?v=2\" %}\">",
    "<link rel=\"stylesheet\" media=\"handheld\" href=\"{\" static \"css/handheld.css?v=2\" %}\">\n"
)]
fn test_formatter_basics(#[case] source: &str, #[case] expected: &str) {
    let config = Config::default();
    let output = format(&config, source);
    assert_eq!(output, expected);
}

/// When an inline closing tag appears mid-line with trailing text, the
/// continuation indent should be preserved for subsequent sibling elements.
/// djlint only unindents for closing tags at the start or end of a line.
#[test]
fn test_inline_close_midline_preserves_indent() {
    let source =
        "<td><a href=\"url\"><b>name1\nname2</b></a> description\n{% if cond %}\n{% for item in items %}\n<a href=\"url2\">{{ item }}</a>\n{% endfor %}\n{% endif %}\n</td>";
    let expected =
        "<td>\n    <a href=\"url\"><b>name1\n        name2</b></a> description\n        {% if cond %}\n            {% for item in items %}<a href=\"url2\">{{ item }}</a>{% endfor %}\n        {% endif %}\n    </td>\n";
    let config = Config::default();
    let output = format(&config, source);
    assert_eq!(output, expected);
}
