//! Phase 2.3b: env-prefix and whitespace differences hash to the same command_key.

use feature_coding_bash::intelligence::command_key;

#[test]
fn env_prefix_and_whitespace_collapse_to_same_key() {
    let a = command_key("cargo test -p agent");
    let b = command_key("  cargo test  -p agent  ");
    let c = command_key("RUST_LOG=debug cargo test -p agent");
    let d = command_key("RUST_LOG=debug  RUST_BACKTRACE=1 cargo test -p agent");
    assert_eq!(a, b);
    assert_eq!(a, c);
    assert_eq!(a, d);
}

#[test]
fn flag_change_produces_different_key() {
    assert_ne!(
        command_key("cargo test -p agent"),
        command_key("cargo test -p agent --nocapture"),
    );
}
