#[cfg(test)]
mod tests {
    use djlintr::config::Config;
    use djlintr::formatter::format;

    /// When inline tag content spans multiple source lines, the formatter
    /// should preserve the line breaks rather than collapsing to one line.
    /// djlint preserves multi-line inline content with proper indentation.
    #[test]
    fn test_multiline_inline_content_preserved() {
        let config = Config::default();

        let input = "<a href=\"{{ url }}\"><b>{{ manufacturer.name }}\n    {{ gear.name }}</b></a> {{ desc }}";
        let output = format(&config, input);

        // The content inside <b> should stay multi-line, not be collapsed
        assert!(
            output.contains("{{ manufacturer.name }}\n"),
            "Expected multi-line content inside <b> to be preserved, but got:\n{}",
            output
        );
        assert!(
            !output.contains("{{ manufacturer.name }}    {{ gear.name }}"),
            "Expected content NOT collapsed to one line, but got:\n{}",
            output
        );
    }
}
