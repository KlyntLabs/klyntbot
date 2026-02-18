//! Types for the execution engine.

use std::time::Duration;

use providers::ChatParams;

/// Parameters controlling a single LLM-tool execution cycle.
#[derive(Debug, Clone)]
pub struct ExecutionParams {
    pub tool_timeout: Duration,
    pub chat_params: ChatParams,
}

impl ExecutionParams {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            tool_timeout: Duration::from_secs(30),
            chat_params: ChatParams::new(model),
        }
    }

    pub fn with_timeout(mut self, dur: Duration) -> Self {
        self.tool_timeout = dur;
        self
    }
}

/// Result of executing a single tool call.
#[derive(Debug)]
pub struct ToolExecutionResult {
    pub tool_call_id: String,
    pub tool_name: String,
    pub result: String,
    pub duration_ms: u64,
    pub success: bool,
}

/// Outcome of a single LLM-tool cycle.
#[derive(Debug)]
pub enum CycleOutcome {
    /// LLM requested tool calls; they were executed and results appended.
    ToolsExecuted { results: Vec<ToolExecutionResult> },
    /// LLM returned a final text response (no tool calls).
    FinalResponse { content: String },
    /// LLM returned an empty response.
    EmptyResponse,
}
