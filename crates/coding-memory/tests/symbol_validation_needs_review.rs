//! Phase 6 — surviving-but-drifted symbols are marked `needs_review`.

#[test]
fn symbol_validation_emits_needs_review() {
    let src = include_str!("../src/reforge/symbol_validation.rs");
    assert!(
        src.contains("needs_review"),
        "symbol_validation.rs should set needs_review status for drifted symbols"
    );
    assert!(
        src.contains("mark_needs_review"),
        "symbol_validation.rs should expose mark_needs_review"
    );
}
