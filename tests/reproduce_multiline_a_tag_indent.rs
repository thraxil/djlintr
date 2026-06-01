use djlintr::{config::Config, formatter::format};

#[test]
fn test_multiline_a_tag_indent() {
    let source = r#"{% if a %}
    <a hx-get="?"
       class="flex items-center py-1 px-3 text-sm rounded-lg hover:text-white hover:cursor-pointer text-default1 hover:bg-default3">
        This is some longer text to prevent collapsing.
    </a>
{% endif %}"#;

    // The bug in djlintr caused the inner text to only be indented 4 spaces instead of 8,
    // and the closing </a> tag to be inlined or incorrectly placed.
    // The expected output should match djlint exactly.
    let expected = r#"{% if a %}
    <a hx-get="?"
       class="flex items-center py-1 px-3 text-sm rounded-lg hover:text-white hover:cursor-pointer text-default1 hover:bg-default3">
        This is some longer text to prevent collapsing.
    </a>
{% endif %}
"#;

    let config = Config::default();
    let formatted = format(&config, source);

    assert_eq!(formatted, expected);
}
