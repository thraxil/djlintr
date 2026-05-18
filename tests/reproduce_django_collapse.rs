#[cfg(test)]
mod tests {
    use djlintr::config::Config;
    use djlintr::formatter::format;

    #[test]
    fn test_django_block_collapse_with_tags() {
        let input = r#"
<h1>Manufacturers {% if user.is_authenticated %}
    (<a href="create/">+</a>)
    {% endif %}</h1>
"#;
        let config = Config {
            indent: 4,
            max_line_length: 120,
            ..Config::default()
        };
        let output = format(&config, input);
        println!("Output: {:?}", output);
        // Should contain collapsed version
        assert!(
            output.contains("{% if user.is_authenticated %}(<a href=\"create/\">+</a>){% endif %}")
        );
    }
}
