use djlintr::{config::Config, formatter::format};

#[test]
fn test_textarea_attribute_wrapping() {
    let source = r#"<textarea class="a" name="b" {% if c %} d="e" {% endif %}>
    {% if f %}{{ g }}{% endif %}
</textarea>"#;

    // textarea should not have its attributes wrapped, even if it exceeds the line length limit
    // it falls under the same category as pre, script, and style blocks for djlint
    let expected = r#"<textarea class="a" name="b" {% if c %} d="e" {% endif %}>
    {% if f %}{{ g }}{% endif %}
</textarea>
"#;

    let config = Config::default();
    let formatted = format(&config, source);

    println!("DEBUG output: {:?}", formatted);
    assert_eq!(formatted, expected);
}
