#[test]
fn expansion_compiles() {
    let t = trybuild::TestCases::new();
    t.pass("tests/expand/noop.rs");
    t.pass("tests/expand/event_basic.rs");
    t.pass("tests/expand/entity_basic.rs");
    t.pass("tests/expand/feature_basic.rs");
}
