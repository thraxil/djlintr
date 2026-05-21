#[cfg(test)]
mod tests {
    use djlintr::config::Config;
    use djlintr::formatter::format;

    /// djlint wraps attributes when the raw attribute string length is >=
    /// max_attribute_length (default 70). The comparison uses strict `<`,
    /// and it measures the raw content before style normalization.
    #[test]
    fn test_attribute_wrap_at_exact_boundary() {
        let config = Config {
            indent: 4,
            max_line_length: 120,
            max_attribute_length: 70,
            ..Config::default()
        };

        // The raw attributes: href="{{ deck.get_absolute_url }}" style="color: #{{ deck.fgcolor }};"
        // That's exactly 70 chars, so djlint wraps (70 >= 70).
        let input = r#"<a href="{{ deck.get_absolute_url }}" style="color: #{{ deck.fgcolor }};">{{ deck.name }}</a>"#;
        let output = format(&config, input);

        // djlint wraps the attributes across two lines
        assert!(
            output.contains("href=\"{{ deck.get_absolute_url }}\"\n"),
            "Expected attributes to wrap onto separate lines, but got:\n{}",
            output
        );
    }

    /// When the raw attribute length is under the threshold, don't wrap.
    #[test]
    fn test_attribute_no_wrap_under_boundary() {
        let config = Config {
            indent: 4,
            max_line_length: 120,
            max_attribute_length: 70,
            ..Config::default()
        };

        // One fewer char: 69 chars of attributes -> no wrapping
        let input = r#"<a href="{{ deck.get_absolute_url }}" style="color: #{{ deck.fgclr }};">{{ deck.name }}</a>"#;
        let output = format(&config, input);

        // Should stay on one line
        assert!(
            !output.contains("\n   "),
            "Expected attributes to stay on one line, but got:\n{}",
            output
        );
    }
}
