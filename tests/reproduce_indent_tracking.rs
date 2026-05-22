#[cfg(test)]
mod tests {
    use djlintr::config::Config;
    use djlintr::formatter::format;

    /// When a non-collapsible inline tag (like a long <a>) is inside a block
    /// tag (like <li>), closing tags after it must be at the correct indent.
    /// The <a> didn't increment (dedup with <li>), so </a> must not decrement.
    #[test]
    fn test_closing_indent_after_non_collapsed_inline() {
        let config = Config::default();

        // This <a> is too long to collapse (>120 chars with indent), so it
        // expands. </li> must stay at indent 1.
        let input = r#"<ul>
<li><a href="{{ musiciangear.get_absolute_url }}">{{ musiciangear.musician.name }} : {{ musiciangear.gear.name }}</a></li>
</ul>"#;
        let output = format(&config, input);

        assert!(
            output.contains("    </li>"),
            "Expected </li> at indent 1 (4 spaces), but got:\n{}",
            output
        );
    }

    /// When an inline tag like <span> follows inline text content ("by ") on
    /// the same output line, the <span> must still increment normally because
    /// text content is NOT an indent increment. The closing </span> must
    /// then decrement.
    #[test]
    fn test_span_after_text_indents_correctly() {
        let config = Config::default();

        let input = r#"<div>
by <span class="author">
<span class="name">
<a href="/user/">username</a>
</span>
</span>
<time>2024</time>
</div>"#;
        let output = format(&config, input);

        // The outer </span> should be at indent 0. djlint always
        // decrements for closing inline tags, even when the opening tag
        // was mid-line and didn't increment. This causes the indent to
        // go below the parent level.
        let lines: Vec<&str> = output.lines().collect();
        let outer_close_span = lines
            .iter()
            .filter(|l| l.trim() == "</span>")
            .last()
            .expect("Should find closing </span>");

        let indent = outer_close_span.len() - outer_close_span.trim_start().len();
        assert_eq!(
            indent, 0,
            "Expected outer </span> at indent 0, got {} spaces.\nFull output:\n{}",
            indent, output
        );
    }
}
