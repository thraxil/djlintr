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
fn test_reformat_parity(#[case] source: &str, #[case] expected: &str) {
    let mut config = Config::default();
    config.profile = "django".to_string();
    config.max_blank_lines = 0;
    let output = format(&config, source);
    assert_eq!(output, expected);
}
