#[cfg(test)]
mod tests {
    use djlintr::config::Config;
    use djlintr::linter::lint;

    #[test]
    fn test_h025_orphan_open_tag() {
        let html = "<div>";
        let config = Config::default();
        let errors = lint(&config, html);

        let h025_errors: Vec<_> = errors.iter().filter(|e| e.code == "H025").collect();
        assert!(
            !h025_errors.is_empty(),
            "Should detect orphan open <div> tag. Errors: {:?}",
            errors
        );
    }

    #[test]
    fn test_h025_orphan_closing_td_tag() {
        let html = "</td>";
        let config = Config::default();
        let errors = lint(&config, html);

        let h025_errors: Vec<_> = errors.iter().filter(|e| e.code == "H025").collect();
        assert_eq!(
            h025_errors.len(),
            1,
            "Should detect orphan </td> tag. Errors: {:?}",
            errors
        );
    }

    #[test]
    fn test_h025_orphan_open_select_tag() {
        let html = "<select name='priority'>";
        let config = Config::default();
        let errors = lint(&config, html);

        let h025_errors: Vec<_> = errors.iter().filter(|e| e.code == "H025").collect();
        assert!(
            !h025_errors.is_empty(),
            "Should detect orphan <select> tag. Errors: {:?}",
            errors
        );
    }
}
