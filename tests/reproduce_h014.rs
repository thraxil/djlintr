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
    fn test_h014_real_behavior() {
        // djlint only flags if there are at least 3 newlines (2 blank lines)
        let html = "line1\n\nline2"; // 1 blank line
        let config = Config::default();
        let errors = lint(&config, html);
        let h014_errors: Vec<_> = errors.iter().filter(|e| e.code == "H014").collect();
        assert!(h014_errors.is_empty(), "Should NOT detect 1 blank line");

        let html = "line1\n\n\nline2"; // 2 blank lines
        let errors = lint(&config, html);
        let h014_errors: Vec<_> = errors.iter().filter(|e| e.code == "H014").collect();
        assert!(!h014_errors.is_empty(), "Should detect 2 blank lines");
    }
}
