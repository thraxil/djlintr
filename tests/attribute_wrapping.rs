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
fn test_complex_attribute_no_split() {
    let mut config = Config::default();
    config.max_attribute_length = 20;
    let input = "<button type=\"button\" @click=\"exportModalOpen = true; $nextTick(() => $refs.exportModal.focus())\">";
    let output = format(&config, input);

    let expected = "<button type=\"button\"\n        @click=\"exportModalOpen = true; $nextTick(() => $refs.exportModal.focus())\">\n";
    assert_eq!(output, expected);
}
