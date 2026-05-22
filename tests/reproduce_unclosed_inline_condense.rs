#[cfg(test)]
mod tests {
    use djlintr::config::Config;
    use djlintr::formatter::format;

    /// djlint condenses {% block %} with unclosed inline tags to one line
    /// when the content fits within max_line_length. The unclosed <span>
    /// should not prevent the condensing.
    #[test]
    fn test_block_with_unclosed_span_condenses() {
        let config = Config::default();

        let input =
            "{% block subhead %}<span itemprop=\"name headline\">{{ post.title }}{% endblock %}";
        let output = format(&config, input);

        assert_eq!(
            output,
            "{% block subhead %}<span itemprop=\"name headline\">{{ post.title }}{% endblock %}\n",
            "Expected unclosed span inside block to condense to one line"
        );
    }

    /// Simple case: {% block %}<span>text{% endblock %}
    #[test]
    fn test_block_with_simple_unclosed_span_condenses() {
        let config = Config::default();

        let input = "{% block foo %}<span>foo{% endblock %}";
        let output = format(&config, input);

        assert_eq!(
            output, "{% block foo %}<span>foo{% endblock %}\n",
            "Expected simple unclosed span to condense"
        );
    }
}
