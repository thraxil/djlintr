use djlintr::{config::Config, format};

#[test]
fn test_spaces_around_django_var() {
    let source = "<p>\n    Total Cards: {{ deck_total }}<br />\n    Unlearned: {{ deck_unlearned }}<br />\n</p>";
    let expected = "<p>\n    Total Cards: {{ deck_total }}\n    <br />\n    Unlearned: {{ deck_unlearned }}\n    <br />\n</p>\n";
    let config = Config::default();
    let output = format(&config, source);
    assert_eq!(output, expected);
}

#[test]
fn test_spaces_around_django_var_simple() {
    let source = "Total Cards: {{ deck_total }}";
    let expected = "Total Cards: {{ deck_total }}\n";
    let config = Config::default();
    let output = format(&config, source);
    assert_eq!(output, expected);
}
