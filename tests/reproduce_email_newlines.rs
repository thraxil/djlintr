use djlintr::{config::Config, format};

#[test]
fn test_email_newlines() {
    let source = "Greetings {{ user }},\n\nYou are receiving this email because you (or someone pretending to be you)\nrequested that your password be reset on {{ site.name }}.";
    // djlint output seems to join everything if it's not a block?
    // Actually djlint --reformat for this file gave:
    /*
    Greetings {{ user }},
    You are receiving this email because you (or someone pretending to be you)
    requested that your password be reset on {{ site.name }}.
        */
    // Wait, it even joined the double newline!

    let config = Config::default();
    let output = format(&config, source);
    println!("Output:\n'{}'", output);
    assert!(output.contains("Greetings {{ user }},\n"));
    assert!(output.contains("on {{ site.name }}.\n"));
}

#[test]
fn test_var_punctuation_inline() {
    let source = "{{ user }},";
    let expected = "{{ user }},\n";
    let config = Config::default();
    let output = format(&config, source);
    assert_eq!(output, expected);
}
