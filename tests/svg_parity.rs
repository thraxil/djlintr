use djlintr::{config::Config, lint};

#[test]
fn test_svg_case_parity() {
    // These should NOT trigger H009 or H010
    let html = r#"
<svg viewBox="0 0 16 16">
    <clipPath id="clip0">
        <path d="M0 0h16v16H0z" />
    </clipPath>
</svg>
"#;
    let config = Config::default();
    let errors = lint(&config, html);

    let h009_errors: Vec<_> = errors.iter().filter(|e| e.code == "H009").collect();
    let h010_errors: Vec<_> = errors.iter().filter(|e| e.code == "H010").collect();

    assert!(
        h009_errors.is_empty(),
        "H009 should not flag clipPath. Found: {:?}",
        h009_errors
    );
    assert!(
        h010_errors.is_empty(),
        "H010 should not flag viewBox. Found: {:?}",
        h010_errors
    );
}

#[test]
fn test_standard_tags_upper_flagged() {
    // These SHOULD trigger H009 and H010
    let html = r#"
<DIV CLASS="foo">
    <P>Hello</P>
</DIV>
"#;
    let config = Config::default();
    let errors = lint(&config, html);

    let h009_errors: Vec<_> = errors.iter().filter(|e| e.code == "H009").collect();
    let h010_errors: Vec<_> = errors.iter().filter(|e| e.code == "H010").collect();

    assert_eq!(
        h009_errors.len(),
        4,
        "Should flag DIV, /DIV, P, /P. Found: {:?}",
        h009_errors
    );
    assert_eq!(
        h010_errors.len(),
        1,
        "Should flag CLASS. Found: {:?}",
        h010_errors
    );
}

#[test]
fn test_mixed_case_not_flagged() {
    // djlint only flags fully uppercase names from its list
    let html = r#"
<Div Class="foo">
    <p>Hello</p>
</Div>
"#;
    let config = Config::default();
    let errors = lint(&config, html);

    let h009_errors: Vec<_> = errors.iter().filter(|e| e.code == "H009").collect();
    let h010_errors: Vec<_> = errors.iter().filter(|e| e.code == "H010").collect();

    assert!(
        h009_errors.is_empty(),
        "H009 should not flag mixed-case Div. Found: {:?}",
        h009_errors
    );
    assert!(
        h010_errors.is_empty(),
        "H010 should not flag mixed-case Class. Found: {:?}",
        h010_errors
    );
}
