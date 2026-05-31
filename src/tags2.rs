pub fn can_have_closing_tag(name: &str, custom_blocks: &[String]) -> bool {
    let name_lower = name.to_lowercase();
    let actual_name = name_lower.strip_prefix("end").unwrap_or(&name_lower);

    matches!(
        actual_name,
        "block"
            | "if"
            | "ifchanged"
            | "for"
            | "with"
            | "autoescape"
            | "filter"
            | "spaceless"
            | "cache"
            | "macro"
            | "call"
            | "set"
            | "localize"
            | "compress"
            | "comment"
            | "verbatim"
            | "language"
            | "thumbnail"
            | "raw"
    ) || custom_blocks
        .iter()
        .any(|b| b.to_lowercase() == actual_name)
}
