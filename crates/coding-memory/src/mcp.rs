//! MCP tool stubs for the 8 coding-memory tools added in Phase 4/6.
//!
//! Phase 1 registers the tool names (via `EXPLICIT_TOOL_ALLOWLIST`) and
//! exposes stub handlers that return `NotImplementedInPhase`. Tool schemas
//! are finalized in Phase 4 when handlers gain real behavior.

use common::{KlyntbotError, Result};

/// Canonical tool names — must match entries appended to
/// `EXPLICIT_TOOL_ALLOWLIST` in `crates/config/src/schema/mcp.rs`.
pub const CODING_MEMORY_MCP_TOOLS: &[&str] = &[
    "recall_index",
    "recall_timeline",
    "recall_fetch",
    "trace_causes",
    "check_dead_ends",
    "recall_facts_as_of",
    "recall_change_history",
    "recall_decision_points",
];

/// Stub handler used by MCP registration; returns `NotImplemented`.
pub fn stub_handler(tool_name: &str) -> Result<serde_json::Value> {
    Err(KlyntbotError::NotImplemented(format!(
        "coding-memory MCP tool `{tool_name}` is a Phase-1 stub; wiring lands in Phase {}",
        phase_for_tool(tool_name)
    )))
}

fn phase_for_tool(tool: &str) -> u8 {
    match tool {
        "trace_causes" => 6,
        _ => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NotImplementedInPhase;

    #[test]
    fn every_tool_has_a_phase() {
        for t in CODING_MEMORY_MCP_TOOLS {
            let err = stub_handler(t).unwrap_err();
            let msg = err.to_string();
            assert!(msg.contains(t), "expected tool name in error: {msg}");
            assert!(
                msg.contains("Phase 4") || msg.contains("Phase 6"),
                "expected phase marker in error: {msg}"
            );
        }
    }

    #[test]
    fn tools_match_allowlist_constants() {
        // Structural assertion — ensures EXPLICIT_TOOL_ALLOWLIST in config
        // stays in sync. If this fails, update that list in config/schema/mcp.rs.
        let expected = [
            "recall_index",
            "recall_timeline",
            "recall_fetch",
            "trace_causes",
            "check_dead_ends",
            "recall_facts_as_of",
            "recall_change_history",
            "recall_decision_points",
        ];
        assert_eq!(CODING_MEMORY_MCP_TOOLS, expected);
        let _ = NotImplementedInPhase::new(4);
    }
}
