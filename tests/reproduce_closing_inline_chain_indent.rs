#[cfg(test)]
mod tests {
    use djlintr::config::Config;
    use djlintr::formatter::format;

    /// When an inline tag like <b> appears mid-line and didn't increment,
    /// its closing </b> followed by a parent closing </a> on the same
    /// source line should use the parent's (unindented) level, matching
    /// djlint's behavior.
    #[test]
    fn test_closing_inline_chain_indent() {
        let config = Config::default();

        let input = "<li><a href=\"/u/\">logged in as <b>\n{% if True %}\nfirst\n{% endif %}\n</b></a></li>";
        let output = format(&config, input);

        // </b></a> should be at indent 1 (parent level), not indent 2
        assert!(
            output.contains("    </b></a>"),
            "Expected </b></a> at indent 1 (4 spaces), but got:\n{}",
            output
        );
    }
}
