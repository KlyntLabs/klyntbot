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
}
