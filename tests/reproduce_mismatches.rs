use djlintr::config::Config;
use djlintr::formatter::format;

#[test]
fn test_404_h1() {
    let config = Config {
        indent: 4,
        max_blank_lines: 0,
        ..Default::default()
    };
    let input = "<h1>Not found <span frown>:(</span></h1>";
    let expected = "<h1>\n    Not found <span frown>:(</span>\n</h1>\n";
    let actual = format(&config, input);
    assert_eq!(actual, expected);
}

#[test]
fn test_500_li() {
    let config = Config {
        indent: 4,
        max_blank_lines: 0,
        ..Default::default()
    };
    let input = "<li><a href=\"#\">Login</a></li>";
    let expected = "<li>\n    <a href=\"#\">Login</a>\n</li>\n";
    let actual = format(&config, input);
    assert_eq!(actual, expected);
}

#[test]
fn test_title_indent() {
    let config = Config {
        indent: 4,
        max_blank_lines: 0,
        ..Default::default()
    };
    let input = "<title>gearspotting:\n    {% block title %}{% endblock %}\n</title>";
    let expected = "<title>gearspotting:\n    {% block title %}{% endblock %}\n</title>\n";
    let actual = format(&config, input);
    assert_eq!(actual, expected);
}
