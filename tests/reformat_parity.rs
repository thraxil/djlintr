use djlintr::{config::Config, format};
use rstest::rstest;

#[rstest]
#[case(
    "<div {% if a > b %} attr=\"v1\" attr=\"v1\" {% endif %}></div>",
    "<div {% if a > b %}attr=\"v1\" attr=\"v1\"{% endif %}></div>\n"
)]
#[case(
    "<div attr=\"v\" {% if a %}></div>",
    "<div attr=\"v\" {% if a %}></div>\n"
)]
#[case(
    "<div {% if a %} attr=\"v\"></div>",
    "<div {% if a %}attr=\"v\"></div>\n"
)]
#[case("{{ var1 }} {{ var2 }}", "{{ var1 }} {{ var2 }}\n")]
#[case(
    "<span>\n    <span>\n        foo\n    </span>\n</span>",
    "<span>\n    <span>foo</span>\n</span>\n"
)]
#[case(
    "{% block foo %}<span>\nfoo\n</span>{% endblock %}",
    "{% block foo %}<span>foo</span>{% endblock %}\n"
)]
#[case(
    "{% block foo %}<span>foo{% endblock %}",
    "{% block foo %}<span>foo{% endblock %}\n"
)]
#[case("</span>\n@ <time>", "</span>\n@ <time>\n")]
fn test_reformat_parity(#[case] source: &str, #[case] expected: &str) {
    let mut config = Config::default();
    config.profile = "django".to_string();
    config.max_blank_lines = 0;
    let output = format(&config, source);
    assert_eq!(output, expected);
}
