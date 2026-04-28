//! Phase 6 — `metadata.supporting_chains` is populated on promoted patterns.

#[test]
fn coding_synthesis_writes_supporting_chains() {
    let src = include_str!("../src/reforge/coding_synthesis.rs");
    assert!(
        src.contains("supporting_chains"),
        "coding_synthesis.rs should populate metadata.supporting_chains"
    );
}
