use djlintr::formatter::tokenizer::Tokenizer;
use djlintr::{config::Config, format};

#[test]
fn test_include_expansion2() {
    let mut config = Config::default();
    config.max_attribute_length = 40;
    config.max_line_length = 120;
    let input = r#"<div class="flex">{% include "a.html" %} {% include "b.html" %} {% include "c.html" %} {% include "d.html" %} {% include "e.html" %} {% include "f.html" %}</div>"#;
    let output = format(&config, input);
    println!("--- output ---\n{}", output);

    let tokens: Vec<_> = Tokenizer::new(&input).collect();
    for t in tokens {
        println!("{:?}", t);
    }
}
