use djlintr::{config::Config, format};

#[test]
fn test_load() {
    let mut config = Config::default();
    config.require_closed_blocks = false;
    let html = "{% load static %}\n<div>test</div>\n";
    let formatted = format(&config, html);
    assert_eq!(formatted, html);
}
