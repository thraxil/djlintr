use djlintr::{config::Config, format};

#[test]
fn test_max_line_length_wrap() {
    let html = "<div class=\"flex justify-center items-center h-full\">{% include \"evaluationquestions/icons/loadingspin_icon.html\" with size_classes=\"w-6 h-6\" stroke_class=\"stroke-default1\" %}</div>\n";
    let expected = "<div class=\"flex justify-center items-center h-full\">\n    {% include \"evaluationquestions/icons/loadingspin_icon.html\" with size_classes=\"w-6 h-6\" stroke_class=\"stroke-default1\" %}\n</div>\n";

    let mut config = Config::default();
    config.max_line_length = 120; // default is 120

    let formatted = format(&config, html);
    assert_eq!(formatted, expected);
}
