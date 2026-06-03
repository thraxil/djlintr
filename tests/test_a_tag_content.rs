use djlintr::{config::Config, format};

#[test]
fn test_a_tag_content() {
    let mut config = Config::default();
    config.max_attribute_length = 40;
    config.max_line_length = 120;
    let input = r#"<div class="short">
    <a href="long-long-long-long-attribute" title="1234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234">content</a>
</div>"#;
    let output = format(&config, input);
    println!("--- output ---\n{}", output);

    // what happens if it's over 120?
    let input_long = r#"<div class="short">
    <a href="long-long-long-long-attribute" title="1234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234">very extremely incredibly long content that definitely exceeds the 120 character limit and forces wrapping onto the next line hopefully</a>
</div>"#;
    let output_long = format(&config, input_long);
    println!("--- output long ---\n{}", output_long);
}
