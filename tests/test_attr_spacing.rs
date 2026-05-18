#[cfg(test)]
mod tests {
    use djlintr::config::Config;
    use djlintr::formatter::format;

    #[test]
    fn test_django_attr_spacing() {
        let input = r#"<div {% if foo %}class="bar"{% endif %}></div>"#;
        let config = Config::default();
        let output = format(&config, input);
        println!("Output:\n{}", output);
        // We want to see if it adds spaces around {% if %}
        // djlint usually produces: <div {% if foo %} class="bar" {% endif %}></div>
        // OR <div {% if foo %}class="bar"{% endif %}></div>
    }

    #[test]
    fn test_img_collapse() {
        let input = r#"<img class="img-polaroid"
 src="{{ photo.photo.get_100h_src }}" />"#;
        let config = Config {
            indent: 4,
            max_line_length: 120,
            ..Config::default()
        };
        let output = format(&config, input);
        println!("Output: {:?}", output);
        let trimmed = output.trim();
        assert!(!trimmed.contains('\n'), "Should be collapsed to one line");
        assert!(
            trimmed.contains("class=\"img-polaroid\" src="),
            "Should have single space between attributes"
        );
    }
}
