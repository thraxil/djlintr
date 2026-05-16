#[cfg(test)]
mod tests {
    use djlintr::config::Config;
    use djlintr::linter::lint;

    #[test]
    fn test_ignored_blocks() {
        // Content inside script/style/pre/textarea should not trigger rules like H021 (inline style)
        let html = r#"
<script>
    var x = '<div style="color:red"></div>';
</script>
<style>
    .foo { color: red; }
</style>
<pre>
    <div style="color:red"></div>
</pre>
<textarea>
    <div style="color:red"></div>
</textarea>
<!-- <div style="color:red"></div> -->
"#;
        let config = Config::default();
        let errors = lint(&config, html);
        let h021_errors: Vec<_> = errors.iter().filter(|e| e.code == "H021").collect();
        assert!(
            h021_errors.is_empty(),
            "H021 should be ignored inside special blocks. Found: {:?}",
            h021_errors
        );
    }

    #[test]
    fn test_h008_restricted_attributes() {
        // H008 should only flag restricted attributes
        let html = r#"<div class='foo' data-foo='bar'></div>"#;
        let config = Config::default();
        let errors = lint(&config, html);
        let codes: Vec<_> = errors.iter().map(|e| e.code.as_str()).collect();
        assert!(
            codes.contains(&"H008"),
            "Should flag class with single quotes"
        );
        // data-foo is not in the restricted list for H008 in djlint
        let h008_matches: Vec<_> = errors.iter().filter(|e| e.code == "H008").collect();
        assert_eq!(
            h008_matches.len(),
            1,
            "Should only flag 'class', not 'data-foo'"
        );
    }

    #[test]
    fn test_h010_lowercase_attributes() {
        // H010 should only flag attribute names, not content
        let html = r#"<div class="UPPERCASE" ID="foo"></div>"#;
        let config = Config::default();
        let errors = lint(&config, html);
        let h010_errors: Vec<_> = errors.iter().filter(|e| e.code == "H010").collect();
        assert_eq!(
            h010_errors.len(),
            1,
            "Should only flag 'ID', not 'UPPERCASE' content. Found: {:?}",
            h010_errors
        );
        assert_eq!(h010_errors[0].match_str, "ID");
    }

    #[test]
    fn test_h020_empty_tag_pair() {
        // H020 should skip whitespace and require no attributes on open tag
        let html = r#"
<div> </div>
<div class="foo"></div>
<span></span>
"#;
        let config = Config::default();
        let errors = lint(&config, html);
        let h020_errors: Vec<_> = errors.iter().filter(|e| e.code == "H020").collect();
        // Should flag first <div> (only whitespace) and <span> (empty)
        // Should NOT flag second <div> (has attributes)
        assert_eq!(
            h020_errors.len(),
            2,
            "Should flag empty div and span. Found: {:?}",
            h020_errors
        );
    }

    #[test]
    fn test_h025_orphan_stack() {
        // H025 should handle nested orphans correctly
        let html = r#"<div><span></div>"#;
        let config = Config::default();
        let errors = lint(&config, html);
        let h025_errors: Vec<_> = errors.iter().filter(|e| e.code == "H025").collect();
        // <span> is opened but never closed. </div> closes <div>.
        // After tokens, <span> remains on stack.
        assert_eq!(h025_errors.len(), 1, "Should detect orphan span");
        assert!(h025_errors[0].match_str.contains("span"));
    }

    #[test]
    fn test_h014_blank_lines_masking() {
        // H014 should not flag blank lines created by template tag masking
        let html = "<div>\n\n\n</div>";
        let config = Config::default();
        let errors = lint(&config, html);
        let h014_errors: Vec<_> = errors.iter().filter(|e| e.code == "H014").collect();
        assert!(
            !h014_errors.is_empty(),
            "Should detect 3 newlines as extra blank lines"
        );

        let html_template = "{%\n\n\n%}";
        let errors = lint(&config, html_template);
        let h014_errors: Vec<_> = errors.iter().filter(|e| e.code == "H014").collect();
        assert!(
            h014_errors.is_empty(),
            "Should not detect blank lines inside template tags"
        );
    }

    #[test]
    fn test_default_false_rules() {
        // Rules like H017 should be disabled by default
        let html = r#"<img src="foo.png">"#;
        let config = Config::default();
        let errors = lint(&config, html);
        let h017_errors: Vec<_> = errors.iter().filter(|e| e.code == "H017").collect();
        assert!(h017_errors.is_empty(), "H017 should be disabled by default");

        // Should be enabled if explicitly included
        let mut config_inc = Config::default();
        config_inc.include.push("H017".to_string());
        let errors = lint(&config_inc, html);
        let h017_errors: Vec<_> = errors.iter().filter(|e| e.code == "H017").collect();
        assert!(
            !h017_errors.is_empty(),
            "H017 should be enabled when included"
        );
    }

    #[test]
    fn test_h012_alpine_syntax_parity() {
        // H012 should not trigger for '=' inside attribute values (e.g. Alpine.js)
        let html = r#"<button @click="isOpen = !isOpen"></button>"#;
        let config = Config::default();
        let errors = lint(&config, html);
        let h012_errors: Vec<_> = errors.iter().filter(|e| e.code == "H012").collect();
        assert!(
            h012_errors.is_empty(),
            "H012 should not flag '=' inside quotes. Found: {:?}",
            h012_errors
        );

        // Should still flag real violations
        let html_bad = r#"<div class = "foo"></div>"#;
        let errors = lint(&config, html_bad);
        let h012_errors: Vec<_> = errors.iter().filter(|e| e.code == "H012").collect();
        assert_eq!(h012_errors.len(), 1, "Should flag space before =");
    }
}
