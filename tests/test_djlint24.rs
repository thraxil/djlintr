#[test]
fn test_djlint24() {
    use djlintr::{config::Config, formatter::format};
    let source =
        "<div class=\"a\"\n          name=\"b\"\n          {% if c %} d=\"e\" {% endif %}>\n</div>";
    let config = Config::default();
    let formatted = format(&config, source);
    println!("Output:\n{}", formatted);
}
