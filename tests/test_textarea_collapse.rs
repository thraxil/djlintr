use djlintr::{config::Config, format};

#[test]
fn test_textarea_collapse() {
    let html = "<textarea>\n    {{ value }}\n</textarea>\n";
    let config = Config::default();
    let formatted = format(&config, html);
    assert_eq!(formatted, html);
}
