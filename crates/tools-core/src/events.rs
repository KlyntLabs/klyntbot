//! Tool-emitted event variants that flow through `RoutingContext.event_tx`.
//!
//! These events are produced by klynt-core tools during execution and consumed
//! by the agent runtime, which translates them into `AgentEvent` for the UI
//! relay. Keeping the enum in `tools-core` avoids a circular dependency with
//! the `agent` crate.

use serde::{Deserialize, Serialize};

/// Events that tools emit during execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
#[non_exhaustive]
pub enum ToolEvent {
    /// A file edit operation completed (edit, write, apply_patch).
    FileEditWithSymbols {
        path: String,
        op: String, // "edit" | "write" | "apply_patch"
        bytes: u64,
        diff_full: String,
        #[serde(rename = "anchoredSymbols")]
        anchored_symbols: Vec<serde_json::Value>,
        #[serde(rename = "lspDiagnosticsDelta")]
        lsp_diagnostics_delta: Vec<serde_json::Value>,
    },

    /// Sandbox policy was applied to a tool execution.
    SandboxPolicyApplied {
        tool: String,
        policy_summary: String,
        policy_hash: String,
        fallback_unsandboxed: bool,
        fs_constraints: Vec<String>,
        network_constraints: Vec<String>,
    },

    /// Plan mode entered or exited.
    PlanModeChanged {
        in_plan_mode: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        plan_id: Option<String>,
    },

    /// An approval decision was requested from the user.
    ApprovalRequested {
        request_id: String,
        tool: String,
        args_hash: String,
        layer: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        rule_matched: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        mirror_history: Option<serde_json::Value>,
        sandbox_summary: String,
        requires_user_input: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        args: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        cwd: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        layer_reason: Option<String>,
    },

    /// An approval decision was resolved.
    ApprovalResolved {
        request_id: String,
        decision: String,
        decision_reason: String,
        #[serde(rename = "latencyMs")]
        latency_ms: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        persisted_rule: Option<String>,
        decided_by: String,
    },

    /// Typed approval request for the coding surface.
    /// Carries a serialized `desktop_shared::coding::ApprovalRequest` as JSON.
    ApprovalRequest {
        id: String,
        payload: serde_json::Value,
    },
}
