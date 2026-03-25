//! Types for the execution engine.

use std::time::Duration;

use providers::{ChatParams, Usage};

/// Parameters controlling a single LLM-tool execution cycle.
#[derive(Debug, Clone)]
pub struct ExecutionParams {
    pub tool_timeout: Duration,
    pub chat_params: ChatParams,
    /// Per-request max iterations (overrides engine default).
    pub max_iterations: u32,
    /// Max fabrication retries before returning fabricated content as-is.
    pub max_fabrication_retries: u32,
    /// Cancellation token for aborting the execution loop.
    pub cancel_token: Option<tokio_util::sync::CancellationToken>,
    /// The original user message that triggered this execution.
    pub original_message: String,
    /// Chain-of-thought planning prompt for complex tasks.
    /// When set, the reactive engine injects this before iteration 1.
    pub planning_prompt: Option<String>,
    /// Context window size in tokens. Used for mid-loop compression threshold.
    pub context_window: usize,
}

impl ExecutionParams {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            tool_timeout: Duration::from_secs(30),
            chat_params: ChatParams::new(model),
            max_iterations: 10,
            max_fabrication_retries: 2,
            cancel_token: None,
            original_message: String::new(),
            planning_prompt: None,
            context_window: 128_000,
        }
    }

    pub fn with_timeout(mut self, dur: Duration) -> Self {
        self.tool_timeout = dur;
        self
    }

    pub fn with_max_iterations(mut self, max: u32) -> Self {
        self.max_iterations = max;
        self
    }

    pub fn with_max_fabrication_retries(mut self, max: u32) -> Self {
        self.max_fabrication_retries = max;
        self
    }

    pub fn with_cancel_token(mut self, token: tokio_util::sync::CancellationToken) -> Self {
        self.cancel_token = Some(token);
        self
    }

    pub fn with_original_message(mut self, msg: String) -> Self {
        self.original_message = msg;
        self
    }

    pub fn with_planning_prompt(mut self, prompt: String) -> Self {
        self.planning_prompt = Some(prompt);
        self
    }

    pub fn with_context_window(mut self, tokens: usize) -> Self {
        self.context_window = tokens;
        self
    }
}

/// Result of executing a single tool call.
#[derive(Debug)]
pub struct ToolExecutionResult {
    pub tool_call_id: String,
    pub tool_name: String,
    pub arguments: serde_json::Value,
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
    /// LLM returned text that looks like a fabricated tool response.
    FabricatedResponse { content: String },
}

/// Accumulate token usage from one cycle into a running total.
pub fn accumulate_usage(total: &mut Usage, cycle: &Usage) {
    total.prompt_tokens += cycle.prompt_tokens;
    total.completion_tokens += cycle.completion_tokens;
    total.total_tokens += cycle.total_tokens;
    total.cache_read_tokens += cycle.cache_read_tokens;
    total.cache_write_tokens += cycle.cache_write_tokens;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execution_params_has_per_request_fields() {
        let params = ExecutionParams::new("mock")
            .with_max_iterations(5)
            .with_max_fabrication_retries(3)
            .with_original_message("hello".to_string());
        assert_eq!(params.max_iterations, 5);
        assert_eq!(params.max_fabrication_retries, 3);
        assert_eq!(params.original_message, "hello");
        assert!(params.cancel_token.is_none());
    }

    #[test]
    fn execution_params_with_cancel_token() {
        let token = tokio_util::sync::CancellationToken::new();
        let params = ExecutionParams::new("mock").with_cancel_token(token.clone());
        assert!(params.cancel_token.is_some());
    }

    #[test]
    fn execution_params_defaults() {
        let params = ExecutionParams::new("mock");
        assert_eq!(params.max_iterations, 10);
        assert_eq!(params.max_fabrication_retries, 2);
        assert!(params.original_message.is_empty());
        assert_eq!(params.context_window, 128_000);
    }

    #[test]
    fn execution_params_with_context_window() {
        let params = ExecutionParams::new("mock").with_context_window(200_000);
        assert_eq!(params.context_window, 200_000);
    }

    #[test]
    fn execution_params_with_planning_prompt() {
        let params = ExecutionParams::new("mock")
            .with_planning_prompt("Create a step-by-step plan.".to_string());
        assert_eq!(
            params.planning_prompt.as_deref(),
            Some("Create a step-by-step plan.")
        );
    }

    #[test]
    fn execution_params_default_no_planning() {
        let params = ExecutionParams::new("mock");
        assert!(params.planning_prompt.is_none());
    }
}
