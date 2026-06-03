use djlintr::{config::Config, format};

#[test]
fn test_attribute_if_wrap_debug() {
    let mut config = Config::default();
    config.max_attribute_length = 40;
    config.max_line_length = 120;
    let input = r#"<div class="{% if T %}very-long-attribute-value-to-force-wrapping-in-djlintr" data-x="{{ v }}{% endif %}"></div>"#;
    let output = format(&config, input);
    println!("--- output ---\n{}", output);

    // Test the parsing manually
    let attr_re = regex::Regex::new(
        r#"([a-zA-Z0-9:@._#*!-]+(?:\s*=\s*(?:"(?:\{\{[\s\S]*?\}\}|\{%[\s\S]*?%\}|[^"])*"|'(?:\{\{[\s\S]*?\}\}|\{%[\s\S]*?%\}|[^'])*'|[^\s>]+))?|\{\{[\s\S]*?\}\}|\{%[\s\S]*?%\})"#,
    ).unwrap();
    let content = &input[4..input.len() - 7];
    println!("Content: {}", content);
    for m in attr_re.find_iter(content) {
        println!("Match: {}", m.as_str());
    }
}
