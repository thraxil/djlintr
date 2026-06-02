#[test]
fn test_djlint44() {
    use djlintr::{config::Config, formatter::format};
    let source = "<b>\nfoo\n</b>";
    let config = Config::default();
    let formatted = format(&config, source);
    println!("Output:\n{}", formatted);
}
