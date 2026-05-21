#[cfg(test)]
mod tests {
    use djlintr::config::Config;
    use djlintr::formatter::format;

    /// The closing </script> tag should be at the same indent level as the
    /// opening <script> tag. The script content itself is verbatim (not
    /// indented by the formatter), but the closing tag must respect the
    /// surrounding indent context.
    #[test]
    fn test_script_closing_tag_indented() {
        let config = Config::default();

        let input = "<div>\n<script>\nvar x = 1;\nvar y = 2;\n</script>\n</div>";
        let output = format(&config, input);

        // <script> should be at indent 1 (inside <div>),
        // </script> should also be at indent 1
        assert!(
            output.contains("    </script>"),
            "Expected </script> to be indented inside <div>, but got:\n{}",
            output
        );
    }
}
