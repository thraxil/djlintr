use djlintr::{config::Config, format};

#[test]
fn test_include_expansion() {
    let mut config = Config::default();
    config.max_attribute_length = 40;
    config.max_line_length = 120;
    let input = r#"<div class="flex">{% include "a.html" %} {% include "b.html" %} {% include "c.html" %} {% include "d.html" %} {% include "e.html" %} {% include "f.html" %}</div>"#;
    let output = format(&config, input);
    assert_eq!(
        output,
        "<div class=\"flex\">\n    {% include \"a.html\" %}\n    {% include \"b.html\" %}\n    {% include \"c.html\" %}\n    {% include \"d.html\" %}\n    {% include \"e.html\" %}\n    {% include \"f.html\" %}\n</div>\n"
    );
}
