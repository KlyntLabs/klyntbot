//! Execution engines for the intent pipeline.
//!
//! Each engine handles one `ExecutionMode`: Direct, Reactive, or Planned.
//! All implement the `ExecutionEngine` trait with a unified `EngineResult`.

use async_trait::async_trait;
use providers::Usage;
use tools::RoutingContext;

use super::router::EscalationContext;
use crate::execution::{ExecutionParams, ReasoningTrace};

pub mod direct;
pub mod planned;
pub mod reactive;

/// Result from an execution engine.
pub enum EngineResult {
    /// Execution completed successfully.
    Complete {
        content: String,
        usage: Usage,
        iterations: u32,
        traces: Vec<ReasoningTrace>,
        tool_name: Option<String>,
    },
    /// Engine needs to escalate to a more capable mode.
    Escalate {
        reason: String,
        carried_context: EscalationContext,
        usage: Usage,
    },
}

/// Unified trait for all execution engines.
#[async_trait]
pub trait ExecutionEngine: Send + Sync {
    /// Execute with the given messages, tools, and parameters.
    async fn execute(
        &self,
        messages: Vec<providers::Message>,
        tools: &[serde_json::Value],
        params: &ExecutionParams,
        ctx: &RoutingContext,
        event_tx: Option<tokio::sync::mpsc::Sender<crate::events::AgentEvent>>,
    ) -> common::Result<EngineResult>;

    /// Human-readable name for this engine mode.
    fn mode(&self) -> &str;
}
