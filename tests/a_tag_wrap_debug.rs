use djlintr::{config::Config, format};

#[test]
fn test_a_tag_wrap_debug() {
    let mut config = Config::default();
    config.max_attribute_length = 40;
    config.max_line_length = 120;
    let input = r#"    <a href="long-long-long-long-attribute"
       title="1234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234">content</a>"#;
    let output = format(&config, input);
    println!(
        "--- output long ---\n{}\n--- expected ---\n{}",
        output, input
    );
}
