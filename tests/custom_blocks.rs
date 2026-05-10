use djlintr::{config::Config, format};

#[test]
fn test_django_indentation() {
    let config = Config::default();
    let input = "{% if True %}\n<p>Hello</p>\n{% endif %}";
    let output = format(&config, input);
    assert_eq!(output, "{% if True %}\n    <p>\n        Hello\n    </p>\n{% endif %}\n");
}

#[test]
fn test_django_url_no_indent() {
    let config = Config::default();
    let input = "{% url 'home' %}\n<p>Hello</p>";
    let output = format(&config, input);
    assert_eq!(output, "{% url 'home' %}\n<p>\n    Hello\n</p>\n");
}

#[test]
fn test_django_else_indentation() {
    let config = Config::default();
    let input = "{% if True %}\n<p>If</p>\n{% else %}\n<p>Else</p>\n{% endif %}";
    let output = format(&config, input);
    assert_eq!(output, "{% if True %}\n    <p>\n        If\n    </p>\n{% else %}\n    <p>\n        Else\n    </p>\n{% endif %}\n");
}

#[test]
fn test_custom_blocks() {
    let mut config = Config::default();
    config.custom_blocks.push("toc".to_string());
    let input = "{% toc %}\n<p>Hello</p>\n{% endtoc %}";
    let output = format(&config, input);
    assert_eq!(output, "{% toc %}\n    <p>\n        Hello\n    </p>\n{% endtoc %}\n");
}
