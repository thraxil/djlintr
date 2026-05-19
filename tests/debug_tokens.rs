#[cfg(test)]
mod tests {
    use djlintr::formatter::tokenizer::Tokenizer;
    #[test]
    fn debug_inline_tokens() {
        let source = std::fs::read_to_string("tests/parity_data/gearspotting/500.html").unwrap();
        let tokens: Vec<_> = Tokenizer::new(&source).collect();
        for (i, token) in tokens.iter().enumerate() {
            println!("{}: {:?}", i, token);
        }
    }
}
