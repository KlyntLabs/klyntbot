#[test]
fn expansion_compiles() {
    let t = trybuild::TestCases::new();
    t.pass("tests/expand/noop.rs");
    t.pass("tests/expand/event_basic.rs");
    t.pass("tests/expand/entity_basic.rs");
    t.pass("tests/expand/coaching_signal.rs");
    t.pass("tests/expand/metric.rs");
    // Note: feature_basic.rs and mirror_snapshot.rs require bus::DomainEvent
    // which isn't available in the proc-macro test context. The AiFeature derive
    // is tested via integration tests in the workspace root.
}
