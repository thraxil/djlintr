use djlintr::{config::Config, format};

#[test]
fn test_attribute_if_wrap() {
    let mut config = Config::default();
    config.max_attribute_length = 40;
    config.max_line_length = 120;
    let input = r#"<div class="{% if T %}very-long-attribute-value-to-force-wrapping-in-djlintr" data-x="{{ v }}{% endif %}"></div>"#;

    config.better_attribute_parsing = false;
    let output_djlint = format(&config, input);
    assert_eq!(output_djlint, "<div class=\"{% if T %}very-long-attribute-value-to-force-wrapping-in-djlintr\" data-x=\"{{ v }}{% endif %}\"></div>\n");

    config.better_attribute_parsing = true;
    let output_better = format(&config, input);
    assert_eq!(output_better, "<div class=\"{% if T %}very-long-attribute-value-to-force-wrapping-in-djlintr\"\n     data-x=\"{{ v }}{% endif %}\"></div>\n");
}
