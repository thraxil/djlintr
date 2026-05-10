use djlintr::{config::Config, lint};
use rstest::rstest;

#[rstest]
#[case("• REVERT</span>", 0)]
#[case("✕</button>", 0)]
#[case("· {{ pct }}%", 0)]
fn test_utf8_tokenizer_safety(#[case] source: &str, #[case] expected_errors: usize) {
    let config = Config::default();
    let output = lint(&config, source);
    // We don't care about the specific errors here, just that it doesn't panic
    assert!(output.len() >= expected_errors);
}
