use djlintr::{config::Config, format};

#[test]
fn test_django_indentation() {
    let config = Config::default();
    let input = "{% if True %}\n<p>Hello</p>\n{% endif %}";
    let output = format(&config, input);
    // djlint collapses single tags inside blocks
    assert_eq!(output, "{% if True %}<p>Hello</p>{% endif %}\n");
}

#[test]
fn test_django_url_no_indent() {
    let config = Config::default();
    let input = "{% url 'home' %}\n<p>Hello</p>";
    let output = format(&config, input);
    assert_eq!(output, "{% url 'home' %}\n<p>Hello</p>\n");
}

#[test]
fn test_django_else_indentation() {
    let config = Config::default();
    let input = "{% if True %}\n<p>If</p>\n{% else %}\n<p>Else</p>\n{% endif %}";
    let output = format(&config, input);
    // djlint expands if-else
    assert_eq!(
        output,
        "{% if True %}\n    <p>If</p>\n{% else %}\n    <p>Else</p>\n{% endif %}\n"
    );
}

#[test]
fn test_custom_blocks() {
    // djlint never collapses custom block tags — they always expand to
    // multiline, even when the content would fit on one line.
    let mut config = Config::default();
    config.custom_blocks.push("toc".to_string());
    let input = "{% toc %}\n<p>Hello</p>\n{% endtoc %}";
    let output = format(&config, input);
    assert_eq!(output, "{% toc %}\n    <p>Hello</p>\n{% endtoc %}\n");
}

#[test]
fn test_custom_blocks_empty_no_collapse() {
    // Even empty custom blocks are never collapsed to one line.
    let mut config = Config::default();
    config.custom_blocks.push("component".to_string());
    let input = "{% component \"x\" %}\n{% endcomponent %}";
    let output = format(&config, input);
    assert_eq!(output, "{% component \"x\" %}\n{% endcomponent %}\n");
}
