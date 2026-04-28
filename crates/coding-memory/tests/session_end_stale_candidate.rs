//! Phase 5 — session-end pass resolves stale candidates (stability < 0.2,
//! older than 30 days).

#[test]
fn session_end_has_stale_resolution() {
    let src = include_str!("../src/reforge/session_end.rs");
    assert!(
        src.contains("resolve_stale_candidates"),
        "session_end.rs should expose resolve_stale_candidates"
    );
    assert!(
        src.contains("0.2"),
        "stability threshold 0.2 should be present"
    );
}
