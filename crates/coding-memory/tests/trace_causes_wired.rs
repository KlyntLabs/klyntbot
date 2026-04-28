//! Phase 4 — `causal_repo` is injected into `CodingRecallService` at boot.

#[test]
fn app_core_injects_causal_repo() {
    let src = include_str!("../../app-core/src/init/mod.rs");
    assert!(
        src.contains("with_causal_repo"),
        "app-core init should call CodingRecallService::with_causal_repo"
    );
}
