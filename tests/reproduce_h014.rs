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
            "Should detect 2 blank lines (3 newlines)"
        );
    }
}
