use djlintr::{config::Config, format};

#[test]
fn test_attribute_wrapping() {
    let mut config = Config::default();
    config.max_attribute_length = 20; // Set low to force wrapping
    let input = "<div class=\"long-class-name\" id=\"my-id\">Hello</div>";
    let output = format(&config, input);

    let expected = "<div\n    class=\"long-class-name\"\n    id=\"my-id\"\n>\n    Hello\n</div>\n";
    assert_eq!(output, expected);
}

#[test]
fn test_no_attribute_wrapping() {
    let mut config = Config::default();
    config.max_attribute_length = 100; // Set high to prevent wrapping
    let input = "<div class=\"short\" id=\"id\">Hello</div>";
    let output = format(&config, input);

    // Note: The current formatter adds a newline after every text/tag/etc.
    // So even without wrapping, <div ...> is followed by \n.
    let expected = "<div class=\"short\" id=\"id\">\n    Hello\n</div>\n";
    assert_eq!(output, expected);
}
