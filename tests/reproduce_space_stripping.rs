use djlintr::{config::Config, format};

#[test]
fn test_spaces_around_inline_tag() {
    let source = "{% block content %}\nThanks, activation complete!  You may now <a href='{% url auth_login %}'>login</a> using the username and password you set at registration.\n{% endblock %}";
    let expected = "{% block content %}\n    Thanks, activation complete!  You may now <a href='{% url auth_login %}'>login</a> using the username and password you set at registration.\n{% endblock %}\n";
    let config = Config::default();
    let output = format(&config, source);
    assert_eq!(output, expected);
}
