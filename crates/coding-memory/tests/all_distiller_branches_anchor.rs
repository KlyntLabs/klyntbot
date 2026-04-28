//! Phase 6 — every distiller fact-write branch (incl. synthetic FixAttempt)
//! extracts `anchored_symbols`.

#[test]
fn distiller_extracts_anchors_in_synthetic_branch() {
    let src = include_str!("../src/distiller/mod.rs");
    let anchored_uses = src.matches("anchored_symbols").count();
    assert!(
        anchored_uses >= 2,
        "distiller/mod.rs should populate anchored_symbols on multiple branches \
         (got {} occurrences)",
        anchored_uses
    );
}
