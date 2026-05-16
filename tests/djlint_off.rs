use djlintr::{config::Config, lint};

#[test]
fn test_djlint_off_block() {
    let source = r#"{# djlint:off #}
<div class='test'>
{# djlint:on #}
<div class='test'>
"#;
    let mut config = Config::default();
    config.profile = "all".to_string();
    let mut output = lint(&config, source);
    output.retain(|e| e.code != "H025");

    // Should only find H008 for the second div
    assert_eq!(output.len(), 1);
    assert_eq!(output[0].code, "H008");
    assert_eq!(output[0].line, 4);
}

#[test]
fn test_djlint_off_rule() {
    let source = r#"{# djlint:off H008 #}
<div class='test'>
<div class="test"></div>
{# djlint:on #}
<div class='test'>
"#;
    let mut config = Config::default();
    config.profile = "all".to_string();
    let output = lint(&config, source);

    // Should only find H008 for the last div.
    // H025 (orphan) might be found if we don't have matching tags, but let's focus on H008.
    let h008_errors: Vec<_> = output.iter().filter(|e| e.code == "H008").collect();
    assert_eq!(h008_errors.len(), 1);
    assert_eq!(h008_errors[0].line, 5);
}

#[test]
fn test_html_comment_off() {
    let source = r#"<!-- djlint:off -->
<div class='test'>
<!-- djlint:on -->
<div class='test'>
"#;
    let mut config = Config::default();
    config.profile = "all".to_string();
    let output = lint(&config, source);

    let h008_errors: Vec<_> = output.iter().filter(|e| e.code == "H008").collect();
    assert_eq!(h008_errors.len(), 1);
    assert_eq!(h008_errors[0].line, 4);
}

#[test]
fn test_djlint_off_multiple_rules() {
    let source = r#"{# djlint:off H008,H009 #}
<DIV class='test'>
{# djlint:on #}
<DIV class='test'>
"#;
    let mut config = Config::default();
    config.profile = "all".to_string();
    let mut output = lint(&config, source);
    output.retain(|e| e.code != "H025");

    // Should find H008 and H009 for the second DIV
    assert_eq!(output.len(), 2);
    assert!(output.iter().any(|e| e.code == "H008" && e.line == 4));
    assert!(output.iter().any(|e| e.code == "H009" && e.line == 4));
}

#[test]
fn test_parity_file() {
    let source = std::fs::read_to_string("tests/parity_data/djlint_off.html").unwrap();
    let mut config = Config::default();
    config.profile = "all".to_string();
    let output = lint(&config, &source);

    // H025 might be reported because the div is not closed.
    // But within the djlint:off block, it should be ignored.
    // Wait, the div IS in the block.
    assert_eq!(output.len(), 0);
}
