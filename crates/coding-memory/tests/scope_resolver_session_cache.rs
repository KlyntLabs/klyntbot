//! Phase 4 — per-session scope cache lives in `recall::scope_resolve`.

#[test]
fn scope_resolver_has_cache_field() {
    let src = include_str!("../src/recall/scope_resolve.rs");
    assert!(src.contains("HashMap"), "scope_resolve should use HashMap");
    assert!(
        src.contains("Mutex"),
        "scope_resolve cache should be Mutex-guarded"
    );
}
