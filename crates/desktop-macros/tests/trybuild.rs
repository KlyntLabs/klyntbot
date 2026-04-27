//! Macro UI tests via the `trybuild` crate. Each `.rs` in `fail/` must
//! produce the exact stderr in the matching `.stderr` file.
//!
//! Pass tests are intentionally omitted — the real `desktop` crate (465+
//! commands) serves as the comprehensive compilation test.

#[test]
fn ui_fail() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/trybuild/fail/*.rs");
}

#[test]
fn ui_raw_fail() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/trybuild/raw_fail/*.rs");
}
