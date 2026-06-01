use djlintr::{config::Config, formatter::tokenizer::Tokenizer};

#[test]
fn test_a_indent() {
    let html = "{% if a %}\n    <a hx-get=\"?\"\n       class=\"flex items-center py-1 px-3 text-sm rounded-lg hover:text-white hover:cursor-pointer text-default1 hover:bg-default3\">\n    This is some longer text to prevent collapsing.\n    </a>\n{% endif %}\n";
    let tokens: Vec<_> = Tokenizer::new(&html).collect();
    for (i, t) in tokens.iter().enumerate() {
        println!("{}: {:?}", i, t);
    }
}
