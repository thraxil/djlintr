#[cfg(test)]
mod tests {
    use djlintr::config::Config;
    use djlintr::formatter::format;

    /// When a Django block tag (like {% for %}) contains a single inline child
    /// element, djlint always collapses it to one line — even if that line
    /// exceeds max_line_length. Our formatter should match this behavior.
    #[test]
    fn test_for_block_collapses_single_child_even_when_over_line_length() {
        // At indent level 8 (32 spaces), this line is ~123 chars which exceeds
        // the default max_line_length of 120, but djlint still collapses it.
        let input = r#"<div>
    <div>
        <div>
            <div>
                <div>
                    <div>
                        <div>
                            <select name="deck">
                                {% for deck in decks %}
                                <option value="{{ deck.name }}">{{ deck.name }}</option>
                                {% endfor %}
                            </select>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    </div>
</div>"#;

        let config = Config {
            indent: 4,
            max_line_length: 120,
            ..Config::default()
        };
        let output = format(&config, input);
        // The {% for %} block should be collapsed to a single line, matching djlint behavior
        assert!(
            output.contains(
                "{% for deck in decks %}<option value=\"{{ deck.name }}\">{{ deck.name }}</option>{% endfor %}"
            ),
            "Expected for-loop to be collapsed to a single line, but got:\n{}",
            output
        );
    }
}
