//! Phase 4 — `domain` arg is forwarded from MCP to `DecisionPointsService`.

#[test]
fn mcp_dispatches_domain_arg() {
    let src = include_str!("../src/mcp.rs");
    assert!(
        src.contains("a.domain.as_deref()"),
        "mcp.rs should forward `domain` to recall_decision_points"
    );
}
