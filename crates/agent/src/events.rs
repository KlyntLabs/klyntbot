//! Agent event types for real-time streaming updates.
//!
//! These events are emitted by the agent loop during processing,
//! allowing consumers (like the CLI) to display real-time progress.

/// Events emitted by the agent loop during processing.
#[derive(Debug, Clone)]
pub enum AgentEvent {
    /// A chunk of content streamed from the LLM.
    ContentChunk(String),

    /// A tool execution has started.
    ToolStart {
        name: String,
        args: serde_json::Value,
    },

    /// A tool execution has completed.
    ToolEnd {
        name: String,
        success: bool,
        duration_ms: u64,
    },

    /// A new agent iteration has started.
    IterationStart { iteration: usize, max: usize },

    /// Pipeline classification step completed.
    ClassificationComplete {
        strategy: String,
        confidence: f32,
        source: String,
        duration_ms: u64,
    },

    /// Context assembly step completed.
    ContextAssembled {
        total_tokens: usize,
        budget: usize,
        duration_ms: u64,
    },

    /// Execution engine selected and starting.
    ExecutionStarted {
        engine: String,
        max_iterations: usize,
    },

    /// Processing is complete with the final accumulated content.
    Done(String),

    /// Internal confidence assessment completed (not shown to user in CLI).
    ConfidenceAssessed { score: f32, action: String },

    /// An error occurred during processing.
    Error(String),

    /// A single plan step completed successfully.
    PlanStepCompleted {
        plan_id: uuid::Uuid,
        step_index: usize,
        result: String,
    },

    /// A plan execution finished (completed or failed).
    PlanCompleted {
        plan_id: uuid::Uuid,
        summary: String,
    },
}
