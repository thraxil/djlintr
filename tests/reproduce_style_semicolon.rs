#[cfg(test)]
mod tests {
    use djlintr::config::Config;
    use djlintr::formatter::format;

    /// djlint only normalizes style attributes (stripping trailing semicolons,
    /// reformatting property separators) when the tag's attributes are being
    /// wrapped across multiple lines. When attributes stay on a single line,
    /// the style value is preserved as-is, including any trailing semicolon.
    #[test]
    fn test_style_trailing_semicolon_preserved_when_not_wrapping() {
        let config = Config::default();

        // This tag fits on one line (attributes under max_attribute_length),
        // so the style attribute should be preserved as-is.
        let input = r#"<div class="spacer" style="height: 10px; width: 1px;"></div>"#;
        let output = format(&config, input);

        assert!(
            output.contains(r#"style="height: 10px; width: 1px;""#),
            "Expected trailing semicolon to be preserved when not wrapping, but got:\n{}",
            output
        );
    }

    /// When attributes ARE wrapped, the style gets reformatted and
    /// the trailing semicolon is stripped (djlint behavior).
    #[test]
    fn test_style_trailing_semicolon_stripped_when_wrapping() {
        let mut config = Config::default();
        config.max_attribute_length = 20; // Force wrapping

        let input = r#"<div class="spacer" style="height: 10px; width: 1px;"></div>"#;
        let output = format(&config, input);

        // When wrapping, djlint strips the trailing semicolon
        assert!(
            output.contains("width: 1px\""),
            "Expected trailing semicolon to be stripped when wrapping, but got:\n{}",
            output
        );
    }
}
