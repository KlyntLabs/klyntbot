//! Types for the agent execution path: results, breakpoints, and summary.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// What went wrong during agent-path processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BreakpointKind {
    RoutingMismatch,
    ToolExecutionFailed,
    LoopTimeout,
    FabricationDetected,
    ClassificationLowConfidence,
    ResponseEmpty,
}

/// A structured failure record from the agent path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentBreakpoint {
    pub kind: BreakpointKind,
    pub message_content: String,
    pub details: String,
    pub day: u32,
    pub phase: String,
}

/// Result of processing a single message through the agent path.
#[derive(Debug, Clone)]
pub struct AgentResult {
    pub selected_skill: String,
    pub mode_used: String,
    pub tool_calls: Vec<String>,
    pub iterations: u32,
    pub response: String,
    pub error: Option<String>,
    pub breakpoints: Vec<AgentBreakpoint>,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub cache_read_tokens: u32,
    pub cache_write_tokens: u32,
    pub cost_usd: f64,
}

/// Expected workflow pattern for cross-feature messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowPattern {
    /// Independent tools executed in parallel.
    Parallel { expected_tools: Vec<String> },
    /// Tools executed sequentially — output of one feeds the next.
    Sequential { chain: Vec<String> },
}

/// Aggregate statistics from the agent path across the entire simulation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentSummary {
    pub total_agent_calls: u32,
    pub successful: u32,
    pub breakpoints: Vec<AgentBreakpoint>,
    pub breakpoints_by_kind: HashMap<String, u32>,
    pub agent_routing_accuracy: f64,
    pub agent_tool_selection: f64,
    pub react_convergence_rate: f64,
    pub avg_react_iterations: f64,
    pub mode_distribution: HashMap<String, u32>,
    pub multi_turn_coherence: f64,
    pub cross_feature_chain_success: f64,
    pub adversarial_resilience: f64,
    pub error_recovery_rate: f64,
    pub total_workflows: u32,
    pub parallel_workflows: u32,
    pub sequential_workflows: u32,
    pub total_adversarial: u32,
    pub total_followups: u32,
}
