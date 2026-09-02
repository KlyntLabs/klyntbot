//! Cohesive tool exposure policy (LLM channels, subagent, MCP).
//!
//! Declared on each [`Tool`](crate::Tool); projections (agent channel filter,
//! subagent toolkit, MCP default set) read from this value rather than
//! inventing membership outside the live registry.

use common::ChannelMask;

/// MCP surface membership for a registry-backed tool.
///
/// Unreviewed tools are [`Forbidden`](Self::Forbidden) (ADR-0102).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum McpExposure {
    /// Included in the MCP auto-default set when the override list is empty.
    Default,
    /// Included only when explicitly named in the user's override list.
    OptIn,
    /// Never exposed via MCP; also rejected if named in an override.
    #[default]
    Forbidden,
}

/// Cohesive exposure axes for a tool.
///
/// Defaults: all LLM channels, not subagent-visible, MCP Forbidden.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExposurePolicy {
    /// Channels in which this tool is visible to the LLM.
    pub llm_channels: ChannelMask,
    /// Whether this tool is projected into autonomous subagent toolkits.
    pub subagent: bool,
    /// MCP server exposure class.
    pub mcp: McpExposure,
}

impl Default for ExposurePolicy {
    fn default() -> Self {
        Self {
            llm_channels: ChannelMask::ALL,
            subagent: false,
            mcp: McpExposure::Forbidden,
        }
    }
}

/// Historical registry-backed MCP Default tool names (EXPO-2.3).
///
/// Authoritative membership is declared on each `Tool` via `exposure_policy`;
/// this list is the locked name set for annotation / migration tests only —
/// not a runtime exposure catalog.
pub const HISTORICAL_MCP_DEFAULT_TOOLS: &[&str] = &[
    "tasks",
    "productivity",
    "notes",
    "learning",
    "language_practice",
    "memory",
    "annotate",
    "cron",
    "alarm",
    "mirror",
    "temporal",
    "launcher",
];

/// EXPO-2.3 named intentional removals — MCP Forbidden (coding-memory stubs).
pub const EXPO_23_FORBIDDEN_STUB_TOOLS: &[&str] = &[
    "recall_index",
    "recall_timeline",
    "recall_fetch",
    "trace_causes",
    "check_dead_ends",
    "recall_facts_as_of",
    "recall_change_history",
    "recall_decision_points",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposure_policy_defaults_all_channels_subagent_false_mcp_forbidden() {
        let policy = ExposurePolicy::default();
        assert_eq!(policy.llm_channels, ChannelMask::ALL);
        assert!(!policy.subagent);
        assert_eq!(policy.mcp, McpExposure::Forbidden);
    }

    #[test]
    fn mcp_exposure_default_is_forbidden() {
        assert_eq!(McpExposure::default(), McpExposure::Forbidden);
    }

    #[test]
    fn historical_mcp_default_name_set_is_locked() {
        assert_eq!(HISTORICAL_MCP_DEFAULT_TOOLS.len(), 12);
        assert!(HISTORICAL_MCP_DEFAULT_TOOLS.contains(&"memory"));
        assert!(HISTORICAL_MCP_DEFAULT_TOOLS.contains(&"tasks"));
        assert!(!HISTORICAL_MCP_DEFAULT_TOOLS.contains(&"agent"));
        for stub in EXPO_23_FORBIDDEN_STUB_TOOLS {
            assert!(
                !HISTORICAL_MCP_DEFAULT_TOOLS.contains(stub),
                "{stub} must not be a historical Default"
            );
        }
    }

    #[test]
    fn expo_23_forbidden_stub_name_set_is_locked() {
        assert_eq!(EXPO_23_FORBIDDEN_STUB_TOOLS.len(), 8);
    }
}
