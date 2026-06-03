use djlintr::{config::Config, format};

#[test]
fn test_include_expansion_with_breaks() {
    let mut config = Config::default();
    config.max_attribute_length = 40;
    config.max_line_length = 120;

    // djlint puts tags like include on their own lines even inside short divs
    let input = r#"<div class="flex">{% include "a.html" %}</div>"#;
    let output = format(&config, input);
    println!("--- output ---\n{}", output);
}
