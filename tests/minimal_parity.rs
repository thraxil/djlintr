use djlintr::{config::Config, format};

fn run_parity_test(source: &str, expected: &str) {
    let mut config = Config::default();
    config.profile = "django".to_string();
    config.max_blank_lines = 0;
    config.indent = 4;
    let output = format(&config, source);
    if output != expected {
        println!("--- ACTUAL ---");
        println!("{}", output);
        println!("--- EXPECTED ---");
        println!("{}", expected);
    }
    assert_eq!(output, expected);
}

#[test]
fn test_attribute_wrapping() {
    let source = "<!-- A: Attribute Wrapping -->
<form action=\"{% url 'gear:gear_add_link' slug=gear.slug %}\" method=\"post\">
    <table>{{ form.as_table }}</table>
</form>";

    let expected = "<!-- A: Attribute Wrapping -->
<form action=\"{% url 'gear:gear_add_link' slug=gear.slug %}\" method=\"post\">
    <table>
        {{ form.as_table }}
    </table>
</form>\n";

    run_parity_test(source, expected);
}

#[test]
fn test_block_one_lining() {
    let source = "<!-- B: Block one-lining -->
{% block subhead %}
    Gear
    {% if user.is_authenticated %}
        (<a href=\"/link/\">+</a>)
    {% endif %}
{% endblock %}";

    let expected = "<!-- B: Block one-lining -->
{% block subhead %}
    Gear
    {% if user.is_authenticated %}
        (<a href=\"/link/\">+</a>)
    {% endif %}
{% endblock %}\n";

    run_parity_test(source, expected);
}

#[test]
fn test_script_tag() {
    let source = "<!-- C: Script tag -->
<script>
jQuery(function() {
    {% if request.user.is_authenticated %}jQuery('#add-manufacturer-form').hide();{% endif %}
});
</script>";

    let expected = "<!-- C: Script tag -->
<script>
jQuery(function() {
    {% if request.user.is_authenticated %}jQuery('#add-manufacturer-form').hide();{% endif %}
});
</script>\n";

    run_parity_test(source, expected);
}

#[test]
fn test_empty_tags() {
    let source = "<!-- D: Empty tags -->
<header>
</header>";

    let expected = "<!-- D: Empty tags -->
<header>
</header>\n";

    run_parity_test(source, expected);
}

#[test]
fn test_inline_tags_with_newlines() {
    let source = "<!-- E: Inline tags with newlines -->
<span>
    <a href=\"/link/\">Link</a>
</span>";

    let expected = "<!-- E: Inline tags with newlines -->
<span>
    <a href=\"/link/\">Link</a>
</span>\n";

    run_parity_test(source, expected);
}

#[test]
fn test_spacing_and_text() {
    let source = "<!-- F: Spacing and text -->
Greetings {{ user }},

To reset your password, please click the following link:
{{ protocol }}://{{ domain }}{% url auth_password_reset_confirm uid token %}

Best regards,
{{ site_name }} Management";

    let expected = "<!-- F: Spacing and text -->
Greetings {{ user }},
To reset your password, please click the following link:
{{ protocol }}://{{ domain }}{% url auth_password_reset_confirm uid token %}
Best regards,
{{ site_name }} Management\n";

    run_parity_test(source, expected);
}
