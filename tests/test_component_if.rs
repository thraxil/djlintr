use djlintr::{config::Config, format};

#[test]
fn test_component_if() {
    let html = "{% if True %}\n    {% component \"my_comp\" / %}\n{% endif %}\n{% if True %}\n    <div>\n        <p>Some content</p>\n    </div>\n{% endif %}\n";
    let expected = "{% if True %}\n    {% component \"my_comp\" / %}\n    {% endif %}\n    {% if True %}\n        <div>\n            <p>Some content</p>\n        </div>\n    {% endif %}\n";

    let mut config = Config::default();
    config.custom_blocks = vec!["component".to_string(), "endcomponent".to_string()];

    let formatted = format(&config, html);
    assert_eq!(formatted, expected);
}
