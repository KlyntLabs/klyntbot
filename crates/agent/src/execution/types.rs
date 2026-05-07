//! Types for the execution engine.

use std::sync::Arc;
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
    /// Maximum wall-clock time for the entire pipeline execution.
    /// When set, `tokio::time::timeout` wraps the router call.
    pub pipeline_timeout: Option<Duration>,
    /// Context window size in tokens. Used for mid-loop compression threshold.
    pub context_window: usize,
    /// Shared queue for mid-execution context updates from cognitive/productivity systems.
    pub context_update_queue: Option<std::sync::Arc<bus::ContextUpdateQueue>>,
    /// When true, the LiveContextRefresher skips injection (frozen-context mode).
    pub pause_context_updates: bool,
    /// Hook engine for firing PreCompact / PostCompact lifecycle hooks.
    pub hook_engine: Option<Arc<klynt_hooks::HookEngine>>,
    /// Global kill switch for cache-breakpoint placement.
    pub cache_enabled: bool,
}

impl ExecutionParams {
    /// Construct execution params with a required `context_window` (in tokens).
    ///
    /// `context_window` MUST come from the provider's `context_window()` method
    /// (e.g. `provider.context_window()`). The mid-loop compressor uses this to
    /// derive its threshold; passing the wrong value silently shrinks context.
    pub fn new(model: impl Into<String>, context_window: usize) -> Self {
        Self {
            tool_timeout: Duration::from_secs(30),
            chat_params: ChatParams::new(model),
            max_iterations: 10,
            max_fabrication_retries: 2,
            cancel_token: None,
            original_message: String::new(),
            planning_prompt: None,
            pipeline_timeout: None,
            context_window,
            context_update_queue: None,
            pause_context_updates: false,
            hook_engine: None,
            cache_enabled: true,
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

    pub fn with_pipeline_timeout(mut self, dur: Duration) -> Self {
        self.pipeline_timeout = Some(dur);
        self
    }

    pub fn with_context_window(mut self, tokens: usize) -> Self {
        self.context_window = tokens;
        self
    }

    pub fn with_context_update_queue(
        mut self,
        queue: std::sync::Arc<bus::ContextUpdateQueue>,
    ) -> Self {
        self.context_update_queue = Some(queue);
        self
    }

    pub fn with_pause_context_updates(mut self, pause: bool) -> Self {
        self.pause_context_updates = pause;
        self
    }

    pub fn with_hook_engine(mut self, engine: Arc<klynt_hooks::HookEngine>) -> Self {
        self.hook_engine = Some(engine);
        self
    }

    pub fn with_cache_enabled(mut self, enabled: bool) -> Self {
        self.cache_enabled = enabled;
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

/// Why an `execute_loop` invocation terminated.
///
/// `Completed` is the success path — the model returned a final response with no tool calls.
/// `Cancelled` indicates user-initiated abort via `CancellationToken`.
/// `SafetyTurnLimit` / `TokenLimit` are silent backstops; hitting one is logged at `error!` for
/// user-facing agents and at `warn!` for subagents (where caps are intentional capability tiers).
/// `LoopDetected` comes from `LoopDetector::HardStop` — repeated identical tool signatures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopFinishReason {
    Completed,
    Cancelled,
    SafetyTurnLimit,
    TokenLimit,
    LoopDetected,
}

impl LoopFinishReason {
    /// Stable wire string. Kept stable for telemetry consumers (analytics, mirror).
    pub fn as_wire_str(&self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::SafetyTurnLimit => "safety_turn_limit_reached",
            Self::TokenLimit => "token_limit_reached",
            Self::LoopDetected => "loop_detected",
        }
    }
}

impl std::fmt::Display for LoopFinishReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_wire_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execution_params_has_per_request_fields() {
        let params = ExecutionParams::new("mock", 128_000)
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
        let params = ExecutionParams::new("mock", 128_000).with_cancel_token(token.clone());
        assert!(params.cancel_token.is_some());
    }

    #[test]
    fn execution_params_defaults() {
        let params = ExecutionParams::new("mock", 128_000);
        assert_eq!(params.max_iterations, 10);
        assert_eq!(params.max_fabrication_retries, 2);
        assert!(params.original_message.is_empty());
        assert_eq!(params.context_window, 128_000);
    }

    #[test]
    fn execution_params_with_context_window() {
        let params = ExecutionParams::new("mock", 128_000).with_context_window(200_000);
        assert_eq!(params.context_window, 200_000);
    }

    #[test]
    fn execution_params_stores_provider_context_window() {
        // Regression guard: Anthropic's 200K window must propagate through
        // construction, not be silently overridden by a default.
        let params = ExecutionParams::new("claude-3-5-sonnet", 200_000);
        assert_eq!(params.context_window, 200_000);
    }

    #[test]
    fn execution_params_with_planning_prompt() {
        let params = ExecutionParams::new("mock", 128_000)
            .with_planning_prompt("Create a step-by-step plan.".to_string());
        assert_eq!(
            params.planning_prompt.as_deref(),
            Some("Create a step-by-step plan.")
        );
    }

    #[test]
    fn execution_params_default_no_planning() {
        let params = ExecutionParams::new("mock", 128_000);
        assert!(params.planning_prompt.is_none());
    }

    #[test]
    fn execution_params_pipeline_timeout_builder() {
        let params = ExecutionParams::new("test-model", 128_000)
            .with_pipeline_timeout(Duration::from_secs(120));
        assert_eq!(params.pipeline_timeout, Some(Duration::from_secs(120)));
    }

    #[test]
    fn execution_params_default_no_pipeline_timeout() {
        let params = ExecutionParams::new("test-model", 128_000);
        assert!(params.pipeline_timeout.is_none());
    }

    #[test]
    fn execution_params_default_no_context_queue() {
        let params = ExecutionParams::new("mock", 128_000);
        assert!(params.context_update_queue.is_none());
        assert!(!params.pause_context_updates);
    }

    #[test]
    fn execution_params_with_context_queue() {
        let queue = std::sync::Arc::new(bus::ContextUpdateQueue::new());
        let params = ExecutionParams::new("mock", 128_000).with_context_update_queue(queue);
        assert!(params.context_update_queue.is_some());
    }

    #[test]
    fn finish_reason_wire_strings_are_stable() {
        assert_eq!(LoopFinishReason::Completed.as_wire_str(), "completed");
        assert_eq!(LoopFinishReason::Cancelled.as_wire_str(), "cancelled");
        assert_eq!(
            LoopFinishReason::SafetyTurnLimit.as_wire_str(),
            "safety_turn_limit_reached"
        );
        assert_eq!(
            LoopFinishReason::TokenLimit.as_wire_str(),
            "token_limit_reached"
        );
        assert_eq!(
            LoopFinishReason::LoopDetected.as_wire_str(),
            "loop_detected"
        );
    }

    #[test]
    fn finish_reason_serializes_snake_case() {
        let json = serde_json::to_string(&LoopFinishReason::SafetyTurnLimit).unwrap();
        assert_eq!(json, "\"safety_turn_limit\"");
    }
}
