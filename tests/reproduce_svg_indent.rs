#[cfg(test)]
mod tests {
    use djlintr::config::Config;
    use djlintr::formatter::format;

    /// SVG container elements like <g>, <defs>, <clipPath> do not indent
    /// their children, but closing these tags must not decrement the indent
    /// level of the surrounding context. Elements after </g> should remain
    /// at the same indent level as <g> itself (inside <svg>).
    #[test]
    fn test_svg_indent_preserved_after_closing_g() {
        let config = Config::default();

        let input = "<svg viewBox=\"0 0 16 16\">\n<g>\n<path d=\"M1 2\" />\n</g>\n<defs>\n<rect width=\"16\" />\n</defs>\n</svg>";
        let output = format(&config, input);

        // All direct children of <svg> should be indented at level 1.
        // <defs> must NOT drop to indent 0 after </g>.
        assert!(
            output.contains("    <defs>"),
            "Expected <defs> to be indented inside <svg>, but got:\n{}",
            output
        );
        assert!(
            output.contains("    </g>"),
            "Expected </g> to be indented inside <svg>, but got:\n{}",
            output
        );
        assert!(
            output.contains("    </defs>"),
            "Expected </defs> to be indented inside <svg>, but got:\n{}",
            output
        );
    }
}
