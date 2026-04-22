#[test]
fn no_bare_event_type_literals() {
    let src_roots = [
        "../cognitive/src/services/reforge/feedback.rs",
        "../agent/src/autotuner/metric_collector.rs",
    ];
    for p in src_roots {
        let body = std::fs::read_to_string(p).unwrap_or_default();
        for line in body.lines() {
            if line.contains("event_type") && line.contains('"')
                && !line.trim_start().starts_with("//")
                && !line.contains("KIND_") {
                panic!("bare event_type literal in {}: {}", p, line);
            }
        }
    }
}
