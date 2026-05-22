#[cfg(test)]
mod tests {
    use djlintr::config::Config;
    use djlintr::formatter::format;

    /// When a Django block is condensed to one line and contains an unclosed
    /// HTML tag (like <span> without </span>), djlint's indent system treats
    /// the unclosed tag as incrementing the indent for subsequent lines.
    /// We need to match this behavior.
    #[test]
    fn test_unclosed_tag_in_condensed_block_leaks_indent() {
        let config = Config::default();

        let input = "{% block subhead %}<span>{{ title }}{% endblock %}\n{% block content %}\n<div>text</div>\n{% endblock %}";
        let output = format(&config, input);

        // {% block content %} should be indented at level 1 because the
        // unclosed <span> in {% block subhead %} leaks indent.
        assert!(
            output.contains("    {% block content %}"),
            "Expected {{% block content %}} at indent 1, but got:\n{}",
            output
        );
    }
}
