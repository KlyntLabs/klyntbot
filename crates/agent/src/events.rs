//! Agent event types for real-time streaming updates.
//!
//! These events are emitted by the agent loop during processing,
//! allowing consumers (like the CLI) to display real-time progress.

use common::PromptRequest;

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

    /// Processing is complete with the final accumulated content.
    Done(String),

    /// An error occurred during processing.
    Error(String),

    /// A tool has requested an interactive prompt from the user.
    /// The CLI should display the prompt and send the response on user_tx.
    PromptUser(PromptRequest),
}
