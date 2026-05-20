//! Stub implementations of the eight `recall_*` coding-memory tools.
//!
//! These are registered in [`ToolKitBuilder`](crate::registry::builder::ToolKitBuilder)
//! so that sub-agents have the tool schema available even when the full
//! `coding-memory` crate is not wired into the toolkit.
//!
//! When `coding-memory` Phase 4 is fully initialised the *live* MCP wrappers
//! (from `coding_memory::CodingMemoryToolset`) are registered in the main
//! agent's tool registry instead; these stubs are then shadowed.

use async_trait::async_trait;
use serde_json::Value;
use tools_core::Tool;

macro_rules! recall_stub {
    ($name:ident, $tool_name:literal, $desc:literal) => {
        pub struct $name;

        #[async_trait]
        impl Tool for $name {
            fn name(&self) -> &str {
                $tool_name
            }
            fn description(&self) -> &str {
                $desc
            }
            fn parameters(&self) -> Value {
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Search query" }
                    }
                })
            }
            async fn execute(
                &self,
                _args: Value,
                _ctx: &tools_core::RoutingContext,
            ) -> common::Result<String> {
                Ok("[recall stub: coding-memory not initialized]".into())
            }
            fn is_concurrency_safe(&self, _args: &Value) -> bool {
                true
            }
            fn allowed_channels(&self) -> common::ChannelMask {
                common::ChannelMask::ALL
            }
        }
    };
}

recall_stub!(
    RecallIndexTool,
    "recall_index",
    "Search coding-memory index"
);
recall_stub!(
    RecallTimelineTool,
    "recall_timeline",
    "Build chronological timeline from coding memory"
);
recall_stub!(
    RecallFetchTool,
    "recall_fetch",
    "Fetch full coding-memory entries by ID"
);
recall_stub!(
    TraceCausesTool,
    "trace_causes",
    "Trace causal graph from a memory entry"
);
recall_stub!(
    CheckDeadEndsTool,
    "check_dead_ends",
    "Check if an approach is a known dead end"
);
recall_stub!(
    RecallFactsAsOfTool,
    "recall_facts_as_of",
    "Query facts as of a specific time"
);
recall_stub!(
    RecallChangeHistoryTool,
    "recall_change_history",
    "Query change history for a subject/predicate"
);
recall_stub!(
    RecallDecisionPointsTool,
    "recall_decision_points",
    "List decision points in coding history"
);
