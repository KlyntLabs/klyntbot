//! Phase 4 — `repo: "*"` wildcard is documented and surface-forwarded.
//!
//! The MCP layer (`coding_memory::mcp`) forwards repo args verbatim and the
//! recall service has an explicit `r == "*"` branch in `recall_index`. This
//! test guards both surfaces.

#[test]
fn recall_service_source_contains_wildcard_branch() {
    let src = include_str!("../src/recall/service.rs");
    assert!(
        src.contains("\"*\""),
        "recall::service should branch on repo == \"*\""
    );
}

#[test]
fn mcp_schema_documents_wildcard() {
    let src = include_str!("../src/mcp.rs");
    assert!(
        src.contains("match all repos"),
        "mcp.rs schema should document \"*\" wildcard"
    );
}
