#[cfg(test)]
mod tests {
    use djlintr::config::Config;
    use djlintr::linter::lint;

    #[test]
    fn test_h014_real_extra_lines() {
        // djlint (Python) flags 2 blank lines (3 newlines)
        let html = "<div></div>\n\n\n<div></div>";
        let config = Config::default();
        let errors = lint(&config, html);

        let h014_errors: Vec<_> = errors.iter().filter(|e| e.code == "H014").collect();
        assert!(
            !h014_errors.is_empty(),
            "Should detect 2 blank lines (3 newlines) with default config"
        );
    }

    #[test]
    fn test_h014_configurable_sensitivity() {
        let html = "<div></div>\n\n<div></div>";
        let mut config = Config::default();

        // Default max_blank_lines is 1, so 1 blank line (2 newlines) is NOT flagged
        let errors = lint(&config, html);
        let h014_errors: Vec<_> = errors.iter().filter(|e| e.code == "H014").collect();
        assert!(h014_errors.is_empty());

        // Set max_blank_lines to 0, now any blank line IS flagged
        config.max_blank_lines = 0;
        let errors = lint(&config, html);
        let h014_errors: Vec<_> = errors.iter().filter(|e| e.code == "H014").collect();
        assert!(
            !h014_errors.is_empty(),
            "Should detect 1 blank line when max_blank_lines is 0"
        );
    }
}
