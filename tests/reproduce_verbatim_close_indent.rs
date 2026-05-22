#[cfg(test)]
mod tests {
    use djlintr::config::Config;
    use djlintr::formatter::format;

    /// When verbatim content (inside <script>) uses tab indentation,
    /// the closing </script> tag should use our space-based indent,
    /// not inherit the tab indentation from the content.
    #[test]
    fn test_script_closing_tag_uses_space_indent() {
        let config = Config::default();

        let input = "<div>\n\t<script>\n\tvar x = 1;\n\tvar y = 2;\n\t</script>\n</div>";
        let output = format(&config, input);

        // </script> should be at indent 1 with spaces, not with tabs
        assert!(
            output.contains("    </script>"),
            "Expected </script> indented with spaces, but got:\n{}",
            output.replace('\t', "→")
        );
        assert!(
            !output.contains("\t</script>"),
            "Expected no tab before </script>, but got:\n{}",
            output.replace('\t', "→")
        );
    }
}
