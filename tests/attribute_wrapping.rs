use djlintr::{config::Config, format};

#[test]
fn test_attribute_wrapping() {
    let mut config = Config::default();
    config.max_attribute_length = 20; // Set low to force wrapping
    let input = "<div class=\"long-class-name\" id=\"my-id\">Hello</div>";
    let output = format(&config, input);

    let expected = "<div class=\"long-class-name\"\n     id=\"my-id\">Hello</div>\n";
    assert_eq!(output, expected);
}

#[test]
fn test_no_attribute_wrapping() {
    let mut config = Config::default();
    config.max_attribute_length = 100; // Set high to prevent wrapping
    let input = "<div class=\"short\" id=\"id\">Hello</div>";
    let output = format(&config, input);

    let expected = "<div class=\"short\" id=\"id\">Hello</div>\n";
    assert_eq!(output, expected);
}
