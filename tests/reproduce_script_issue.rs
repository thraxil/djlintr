#[cfg(test)]
mod tests {
    use djlintr::config::Config;
    use djlintr::formatter::format;

    #[test]
    fn test_script_collapse() {
        let input = r#"
<div id="content-main">
<script type="text/javascript">
document.getElementById('id_username').focus()
</script>
</div>
"#;
        let config = Config {
            indent: 4,
            max_line_length: 120,
            ..Config::default()
        };
        let output = format(&config, input);
        println!("Output:\n{}", output);
        // djlint (Python) would produce:
        // <div id="content-main">
        //     <script type="text/javascript">document.getElementById('id_username').focus()</script>
        // </div>
        assert!(output.contains("    <script type=\"text/javascript\">document.getElementById('id_username').focus()</script>"));
    }

    #[test]
    fn test_verbatim_indent() {
        let input = r#"
<div>
    <script>
        var x = 1;
        // This is a long script that should not be collapsed because it will exceed the maximum line length of 120 characters after we add enough text here.
    </script>
</div>
"#;
        let config = Config {
            indent: 4,
            max_line_length: 120,
            ..Config::default()
        };
        let output = format(&config, input);
        println!("Output:\n{}", output);
        assert!(output.contains("    <script>"));
        assert!(output.contains("    </script>"));
    }
}
