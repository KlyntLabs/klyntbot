//! Phase 4 — `causal_repo` is injected into `CodingRecallService` at boot.

#[test]
fn app_core_injects_causal_repo() {
    // Wiring lives in the dedicated coding_recall init module; check there.
    let src = include_str!("../../app-core/src/init/coding_recall.rs");
    assert!(
        src.contains("with_causal_repo"),
        "app-core::init::coding_recall should call CodingRecallService::with_causal_repo"
    );
}
