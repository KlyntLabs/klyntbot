//! Agent event types for real-time streaming updates.
//!
//! These events are emitted by the agent loop during processing,
//! allowing consumers (like the CLI) to display real-time progress.

use serde::Serialize;

/// Events emitted by the agent loop during processing.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AgentEvent {
    /// A chunk of content streamed from the LLM.
    ContentChunk { data: String },

    /// A tool execution has started.
    ToolStart {
        name: String,
        args: serde_json::Value,
    },

    /// A tool execution has completed.
    ToolEnd {
        name: String,
        success: bool,
        #[serde(rename = "durationMs")]
        duration_ms: u64,
    },

    /// A new agent iteration has started.
    IterationStart { iteration: usize, max: usize },

    /// Pipeline classification step completed.
    ClassificationComplete {
        strategy: String,
        confidence: f32,
        source: String,
        #[serde(rename = "durationMs")]
        duration_ms: u64,
    },

    /// Context assembly step completed.
    ContextAssembled {
        #[serde(rename = "totalTokens")]
        total_tokens: usize,
        budget: usize,
        #[serde(rename = "durationMs")]
        duration_ms: u64,
    },

    /// Execution engine selected and starting.
    ExecutionStarted {
        engine: String,
        #[serde(rename = "maxIterations")]
        max_iterations: usize,
    },

    /// Processing is complete with the final accumulated content.
    Done { content: String },

    /// Internal confidence assessment completed (not shown to user in CLI).
    ConfidenceAssessed { score: f32, action: String },

    /// An error occurred during processing.
    Error { message: String },

    /// A single plan step completed successfully.
    PlanStepCompleted {
        #[serde(rename = "planId")]
        plan_id: uuid::Uuid,
        #[serde(rename = "stepIndex")]
        step_index: usize,
        result: String,
    },

    /// A plan execution finished (completed or failed).
    PlanCompleted {
        #[serde(rename = "planId")]
        plan_id: uuid::Uuid,
        summary: String,
    },
}
