use desktop::specta_builder::build_specta;

#[test]
fn builder_yields_a_handler() {
    let builder = build_specta();
    // The handler is a closure; we just need to confirm the builder loads.
    let _ = builder.invoke_handler();
}
