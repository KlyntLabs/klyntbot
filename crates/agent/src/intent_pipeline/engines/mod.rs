//! Execution engines for the intent pipeline.
//!
//! Each engine handles one `ExecutionMode`: Direct or Reactive.
//! All implement the `ExecutionEngine` trait with a unified `EngineResult`.

use async_trait::async_trait;
use providers::Usage;
use tools::RoutingContext;

use crate::execution::{ExecutionParams, ReasoningTrace};

pub mod debate;
pub mod debate_types;
pub mod direct;
pub mod interaction;
pub mod reactive;
#[cfg(test)]
pub(crate) mod test_utils;

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
    /// DirectEngine detected tool calls — signals the router to escalate to
    /// Reactive mode with the original messages and tools.
    Escalate { usage: Usage },
}

impl EngineResult {
    /// Construct a `Complete` result with no traces or tool name.
    ///
    /// Used by Direct and Planned engines which don't track reasoning traces.
    pub fn complete(content: String, usage: Usage, iterations: u32) -> Self {
        Self::Complete {
            content,
            usage,
            iterations,
            traces: vec![],
            tool_name: None,
        }
    }

    /// Construct an empty `Complete` result.
    pub fn empty(usage: Usage, iterations: u32) -> Self {
        Self::complete(String::new(), usage, iterations)
    }
}

// Derive Debug to enable assertions in tests.
impl std::fmt::Debug for EngineResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Complete {
                content,
                iterations,
                tool_name,
                ..
            } => f
                .debug_struct("Complete")
                .field("content_len", &content.len())
                .field("iterations", iterations)
                .field("tool_name", tool_name)
                .finish(),
            Self::Escalate { .. } => f.debug_struct("Escalate").finish(),
        }
    }
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
