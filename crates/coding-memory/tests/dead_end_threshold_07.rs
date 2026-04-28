//! Phase 4 — dead-end aggregate-confidence threshold raised from 0.5 → 0.7.

#[test]
fn renderers_threshold_is_07() {
    let src = include_str!("../src/recall/renderers.rs");
    assert!(
        src.contains("aggregate_confidence > 0.7"),
        "dead-end threshold should be > 0.7"
    );
    assert!(
        !src.contains("aggregate_confidence > 0.5"),
        "old 0.5 threshold should be gone"
    );
}
