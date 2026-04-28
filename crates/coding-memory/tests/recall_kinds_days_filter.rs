//! Phase 4 — verifies `recall_index` accepts `kinds` and `days` filter args.

#[test]
fn recall_index_signature_takes_kinds_and_days() {
    let src = include_str!("../src/recall/service.rs");
    assert!(
        src.contains("pub async fn recall_index"),
        "recall_index should be a public async fn"
    );
    assert!(
        src.contains("kinds:") && src.contains("days:"),
        "recall_index signature should declare `kinds` and `days` params"
    );
}
