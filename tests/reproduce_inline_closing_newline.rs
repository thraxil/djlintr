#[cfg(test)]
mod tests {
    use djlintr::config::Config;
    use djlintr::formatter::format;

    /// When an inline tag like <a> has children on separate lines (because
    /// it couldn't collapse), the closing </a> should be on its own line
    /// at the opening tag's indent level, not appended to the last child.
    #[test]
    fn test_closing_inline_tag_on_own_line_when_expanded() {
        let config = Config::default();

        // Attributes must be long enough to wrap (exceed max_attribute_length=70)
        let input = r#"<div>
<a href="{{ photo.photo.get_absolute_url }}"
title="{{ photo.photo.caption }}">
<img class="img-polaroid" src="{{ photo.photo.get_100h_src }}" /></a>
</div>"#;

        let output = format(&config, input);

        // </a> should be on its own line, not after <img ... />
        assert!(
            output.contains("    </a>"),
            "Expected </a> on its own indented line, but got:\n{}",
            output
        );
        assert!(
            !output.contains("/></a>"),
            "Expected </a> NOT immediately after />, but got:\n{}",
            output
        );
    }
}
