//! Core execution engine that drives a single LLM→tool cycle.

use std::sync::Arc;
use std::time::Instant;

use futures_util::future::join_all;
use tokio::sync::RwLock;

use common::Result;
use providers::{tool_calls_to_messages, DynProvider, Message};
use tools::{registry::ToolRegistry, RoutingContext};
use tracing::debug;

use providers::Usage;

use crate::execution::types::{CycleOutcome, ExecutionParams, ToolExecutionResult};

/// Detect if a text response is actually a fabricated tool response.
///
/// Some LLMs (DeepSeek, Kimi, etc.) skip tool calls and generate text that
/// looks like a tool result. This function uses heuristics to detect that pattern.
///
/// Returns `true` if the text appears to be a fabricated tool execution.
fn is_fabricated_tool_response(text: &str, tool_names: &[&str]) -> bool {
    if tool_names.is_empty() {
        return false;
    }

    let lower = text.to_lowercase();

    // Pattern 1: Contains a fake ID pattern (hex ID like "9c4e5f3b" or "a1b2c3d4")
    let has_fake_id = {
        let mut found = false;
        // Look for "ID:" or "(ID:" followed by hex-like string
        for pattern in &["id:", "(id:"] {
            if let Some(pos) = lower.find(pattern) {
                let after = &lower[pos + pattern.len()..];
                let trimmed = after.trim_start();
                // Check if next chars are hex-like (at least 6 hex chars)
                let hex_chars = trimmed
                    .chars()
                    .take(10)
                    .take_while(|c| c.is_ascii_hexdigit())
                    .count();
                if hex_chars >= 6 {
                    found = true;
                    break;
                }
            }
        }
        found
    };

    // Pattern 2: Structured result patterns that indicate fabricated output
    let structured_result_indicators = [
        "task created",
        "task added",
        "i've created the task",
        "i searched the web",
        "search results:",
        "here are the results",
        "i found these results",
        "event created",
        "reminder set",
        "calendar event added",
    ];
    let has_structured_result = structured_result_indicators
        .iter()
        .any(|p| lower.contains(p));

    // Pattern 3: Has multiple field-like patterns (Priority:, Due Date:, Description:, Tags:)
    let field_patterns = [
        "priority:",
        "due date:",
        "description:",
        "tags:",
        "estimated time:",
    ];
    let field_count = field_patterns
        .iter()
        .filter(|p| lower.contains(**p))
        .count();
    let has_multiple_fields = field_count >= 2;

    // Pattern 4: Numbered list after a search indicator (e.g. "1. Result\n2. Result")
    let search_indicators = [
        "i searched the web",
        "search results:",
        "here are the results",
        "i found these results",
    ];
    let has_search_with_list = search_indicators.iter().any(|p| lower.contains(p))
        && lower.contains("\n1.")
        && lower.contains("\n2.");

    // Decision: fabricated if (has fake ID AND structured result)
    //   OR (structured result AND multiple fields)
    //   OR search with numbered list
    (has_fake_id && has_structured_result)
        || (has_structured_result && has_multiple_fields)
        || has_search_with_list
}

/// Drives a single LLM-tool execution cycle.
///
/// Given a provider and tool registry, `run_cycle` sends messages to the LLM,
/// executes any requested tool calls in parallel (with per-tool timeout),
/// appends results to the message history, and returns the cycle outcome.
pub struct ExecutionCore {
    pub provider: DynProvider,
    pub tool_registry: Arc<RwLock<ToolRegistry>>,
}

impl ExecutionCore {
    pub fn new(provider: DynProvider, tool_registry: Arc<RwLock<ToolRegistry>>) -> Self {
        Self {
            provider,
            tool_registry,
        }
    }

    /// Run a single LLM-tool cycle:
    /// 1. Call provider.chat() with the current messages and tool definitions
    /// 2. If tool calls returned: execute all in parallel with timeout → ToolsExecuted
    /// 3. If text response: FinalResponse
    /// 4. Otherwise: EmptyResponse
    pub async fn run_cycle(
        &self,
        messages: &mut Vec<Message>,
        tools: &[serde_json::Value],
        params: &ExecutionParams,
        routing_ctx: &RoutingContext,
    ) -> Result<(CycleOutcome, Usage)> {
        let response = self
            .provider
            .chat(messages, Some(tools), &params.chat_params)
            .await?;

        let usage = response.usage.clone();

        if !response.tool_calls.is_empty() {
            debug!("ExecutionCore: LLM returned {} tool calls: {:?}",
                response.tool_calls.len(),
                response.tool_calls.iter().map(|tc| &tc.name).collect::<Vec<_>>()
            );
            // Append assistant message with tool calls
            let tool_call_msgs = tool_calls_to_messages(&response.tool_calls);
            messages.push(Message::assistant_with_tools(tool_call_msgs));

            // Execute all tools in parallel, each with a timeout
            let futures: Vec<_> = response
                .tool_calls
                .iter()
                .map(|tc| {
                    let registry = self.tool_registry.clone();
                    let name = tc.name.clone();
                    let args = tc.arguments.clone();
                    let ctx = routing_ctx.clone();
                    let id = tc.id.clone();
                    let timeout_dur = params.tool_timeout;

                    async move {
                        let start = Instant::now();
                        let exec_result = tokio::time::timeout(timeout_dur, async {
                            let reg = registry.read().await;
                            reg.execute(&name, args, &ctx).await
                        })
                        .await;
                        let duration_ms = start.elapsed().as_millis() as u64;

                        let (result_str, success) = match exec_result {
                            Ok(Ok(s)) => (s, true),
                            Ok(Err(e)) => (format!("Error: {}", e), false),
                            Err(_) => (
                                format!(
                                    "Tool '{}' timed out after {}s",
                                    name,
                                    timeout_dur.as_secs()
                                ),
                                false,
                            ),
                        };

                        ToolExecutionResult {
                            tool_call_id: id,
                            tool_name: name,
                            result: result_str,
                            duration_ms,
                            success,
                        }
                    }
                })
                .collect();

            let results = join_all(futures).await;

            // Append tool result messages
            for r in &results {
                messages.push(Message::tool(
                    r.tool_call_id.clone(),
                    r.tool_name.clone(),
                    r.result.clone(),
                ));
            }

            return Ok((CycleOutcome::ToolsExecuted { results }, usage));
        }

        // No tool calls — check for text content
        debug!("ExecutionCore: LLM returned text response (no tool calls)");
        if let Some(content) = response.content {
            if !content.trim().is_empty() {
                // Extract tool names from the tool definitions for fabrication check
                let tool_names: Vec<&str> = tools
                    .iter()
                    .filter_map(|t| {
                        t.get("function")
                            .and_then(|f| f.get("name"))
                            .and_then(|n| n.as_str())
                    })
                    .collect();

                if !tool_names.is_empty()
                    && is_fabricated_tool_response(&content, &tool_names)
                {
                    debug!(
                        "ExecutionCore: detected fabricated tool response (tools available: {:?})",
                        tool_names
                    );
                    return Ok((CycleOutcome::FabricatedResponse { content }, usage));
                }

                return Ok((CycleOutcome::FinalResponse { content }, usage));
            }
        }

        Ok((CycleOutcome::EmptyResponse, usage))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use providers::{ChatParams, LlmProvider, LlmResponse, ToolCall, Usage};
    use serde_json::Value;
    use std::sync::Mutex;
    use std::time::Duration;
    use tools::Tool;

    // ── Mock provider ────────────────────────────────────────────

    struct MockProvider {
        responses: Mutex<Vec<LlmResponse>>,
    }

    impl MockProvider {
        fn with_text(text: &str) -> Arc<Self> {
            Arc::new(Self {
                responses: Mutex::new(vec![LlmResponse {
                    content: Some(text.to_string()),
                    tool_calls: vec![],
                    finish_reason: "stop".to_string(),
                    usage: Usage::default(),
                    reasoning_content: None,
                }]),
            })
        }

        fn with_tool_call(name: &str, args: Value) -> Arc<Self> {
            Arc::new(Self {
                responses: Mutex::new(vec![LlmResponse {
                    content: None,
                    tool_calls: vec![ToolCall {
                        id: "call_1".to_string(),
                        name: name.to_string(),
                        arguments: args,
                    }],
                    finish_reason: "tool_calls".to_string(),
                    usage: Usage::default(),
                    reasoning_content: None,
                }]),
            })
        }
    }

    #[async_trait]
    impl LlmProvider for MockProvider {
        async fn chat(
            &self,
            _messages: &[Message],
            _tools: Option<&[Value]>,
            _params: &ChatParams,
        ) -> common::Result<LlmResponse> {
            let mut responses = self.responses.lock().unwrap();
            Ok(responses.remove(0))
        }

        fn default_model(&self) -> &str {
            "mock"
        }
        fn name(&self) -> &str {
            "mock"
        }
    }

    // ── Mock tool (fast) ─────────────────────────────────────────

    struct EchoTool;

    #[async_trait]
    impl Tool for EchoTool {
        fn name(&self) -> &str {
            "echo"
        }
        fn description(&self) -> &str {
            "Echoes input"
        }
        fn parameters(&self) -> Value {
            serde_json::json!({"type": "object", "properties": {}})
        }
        async fn execute(&self, args: Value, _ctx: &RoutingContext) -> common::Result<String> {
            Ok(format!("echoed: {}", args))
        }
    }

    // ── Mock tool (slow, for timeout test) ───────────────────────

    struct SlowTool;

    #[async_trait]
    impl Tool for SlowTool {
        fn name(&self) -> &str {
            "slow_tool"
        }
        fn description(&self) -> &str {
            "Sleeps forever"
        }
        fn parameters(&self) -> Value {
            serde_json::json!({"type": "object", "properties": {}})
        }
        async fn execute(&self, _args: Value, _ctx: &RoutingContext) -> common::Result<String> {
            tokio::time::sleep(Duration::from_secs(5)).await;
            Ok("should not reach".to_string())
        }
    }

    // ── Helpers ──────────────────────────────────────────────────

    fn make_registry_with(tool: impl Tool + 'static) -> Arc<RwLock<ToolRegistry>> {
        let mut reg = ToolRegistry::new();
        reg.register(tool);
        Arc::new(RwLock::new(reg))
    }

    fn routing_ctx() -> RoutingContext {
        RoutingContext::new("test".into(), "test".into())
    }

    // ── Tests ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_cycle_final_response() {
        let provider = MockProvider::with_text("Hello world");
        let registry = make_registry_with(EchoTool);
        let core = ExecutionCore::new(provider, registry);

        let mut messages = vec![Message::user("hi")];
        let params = ExecutionParams::new("mock");
        let tools = vec![];

        let (outcome, _usage) = core
            .run_cycle(&mut messages, &tools, &params, &routing_ctx())
            .await
            .unwrap();

        match outcome {
            CycleOutcome::FinalResponse { content } => {
                assert_eq!(content, "Hello world");
            }
            other => panic!("Expected FinalResponse, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_cycle_tool_execution() {
        let provider = MockProvider::with_tool_call("echo", serde_json::json!({"msg": "test"}));
        let registry = make_registry_with(EchoTool);
        let core = ExecutionCore::new(provider, registry);

        let mut messages = vec![Message::user("use echo")];
        let params = ExecutionParams::new("mock");
        let tools = vec![];

        let (outcome, _usage) = core
            .run_cycle(&mut messages, &tools, &params, &routing_ctx())
            .await
            .unwrap();

        match outcome {
            CycleOutcome::ToolsExecuted { results } => {
                assert_eq!(results.len(), 1);
                assert_eq!(results[0].tool_name, "echo");
                assert!(results[0].success);
                assert!(results[0].result.contains("echoed"));
            }
            other => panic!("Expected ToolsExecuted, got {:?}", other),
        }

        // Messages should have assistant + tool result appended
        assert!(messages.len() >= 3); // user + assistant(tool_calls) + tool result
    }

    #[tokio::test]
    async fn test_tool_timeout() {
        let provider = MockProvider::with_tool_call("slow_tool", serde_json::json!({}));
        let registry = make_registry_with(SlowTool);
        let core = ExecutionCore::new(provider, registry);

        let mut messages = vec![Message::user("run slow")];
        let params = ExecutionParams::new("mock").with_timeout(Duration::from_millis(100));
        let tools = vec![];

        let (outcome, _usage) = core
            .run_cycle(&mut messages, &tools, &params, &routing_ctx())
            .await
            .unwrap();

        match outcome {
            CycleOutcome::ToolsExecuted { results } => {
                assert_eq!(results.len(), 1);
                assert!(!results[0].success);
                assert!(results[0].result.contains("timed out"));
            }
            other => panic!("Expected ToolsExecuted with timeout, got {:?}", other),
        }
    }

    // ── Fabrication detection tests ─────────────────────────────

    #[test]
    fn test_detects_fabricated_todo_response() {
        let tool_names = vec!["todo", "calendar", "web_search"];
        let fabricated = "I've created the task for you:\n\n**Task Created:** Buy groceries (ID: 9c4e5f3b)\n- **Description:** Weekly shopping\n- **Priority:** P3 (Medium)\n- **Due Date:** Tomorrow";
        assert!(is_fabricated_tool_response(fabricated, &tool_names));
    }

    #[test]
    fn test_detects_fabricated_search_response() {
        let tool_names = vec!["todo", "calendar", "web_search"];
        let fabricated = "I searched the web for you and found these results:\n1. Rust programming language\n2. Rust game";
        assert!(is_fabricated_tool_response(fabricated, &tool_names));
    }

    #[test]
    fn test_does_not_flag_normal_response() {
        let tool_names = vec!["todo", "calendar", "web_search"];
        let normal = "Sure! I'd be happy to help you create a task. What would you like the task to be about?";
        assert!(!is_fabricated_tool_response(normal, &tool_names));
    }

    #[test]
    fn test_does_not_flag_explanation_about_tools() {
        let tool_names = vec!["todo", "calendar", "web_search"];
        let explanation = "I have access to a todo tool that can help you manage tasks. Would you like me to create one?";
        assert!(!is_fabricated_tool_response(explanation, &tool_names));
    }

    #[test]
    fn test_detects_fabricated_with_fake_id() {
        let tool_names = vec!["todo"];
        let fabricated = "Task added! ID: a1b2c3d4. Title: Buy milk. Priority: High.";
        assert!(is_fabricated_tool_response(fabricated, &tool_names));
    }

    #[test]
    fn test_no_tools_means_no_fabrication() {
        let tool_names: Vec<&str> = vec![];
        let text = "Task Created: Buy groceries (ID: 9c4e5f3b)";
        assert!(!is_fabricated_tool_response(text, &tool_names));
    }

    #[tokio::test]
    async fn test_cycle_empty_response() {
        let provider = Arc::new(MockProvider {
            responses: Mutex::new(vec![LlmResponse {
                content: Some("".to_string()),
                tool_calls: vec![],
                finish_reason: "stop".to_string(),
                usage: Usage::default(),
                reasoning_content: None,
            }]),
        });
        let registry = make_registry_with(EchoTool);
        let core = ExecutionCore::new(provider, registry);

        let mut messages = vec![Message::user("empty")];
        let params = ExecutionParams::new("mock");
        let tools = vec![];

        let (outcome, _usage) = core
            .run_cycle(&mut messages, &tools, &params, &routing_ctx())
            .await
            .unwrap();

        assert!(matches!(outcome, CycleOutcome::EmptyResponse));
    }

    // ── Fabrication detection integration tests ──────────────────

    #[tokio::test]
    async fn test_cycle_detects_fabricated_response() {
        // Provider returns text that looks like a fabricated todo result
        let provider = Arc::new(MockProvider {
            responses: Mutex::new(vec![LlmResponse {
                content: Some(
                    "I've created the task for you:\n\n**Task Created:** Buy groceries (ID: 9c4e5f3b)\n- **Priority:** P3\n- **Due Date:** Tomorrow".to_string()
                ),
                tool_calls: vec![],
                finish_reason: "stop".to_string(),
                usage: Usage::default(),
                reasoning_content: None,
            }]),
        });
        let registry = make_registry_with(EchoTool);
        let core = ExecutionCore::new(provider, registry);

        let mut messages = vec![Message::user("create task: buy")];
        let params = ExecutionParams::new("mock");
        let tools = vec![serde_json::json!({
            "type": "function",
            "function": {
                "name": "todo",
                "description": "Manage tasks",
                "parameters": {"type": "object", "properties": {}}
            }
        })];

        let (outcome, _usage) = core
            .run_cycle(&mut messages, &tools, &params, &routing_ctx())
            .await
            .unwrap();

        assert!(matches!(outcome, CycleOutcome::FabricatedResponse { .. }));
    }

    #[tokio::test]
    async fn test_cycle_normal_text_not_flagged() {
        let provider = MockProvider::with_text("Sure, I can help you create a task. What would you like?");
        let registry = make_registry_with(EchoTool);
        let core = ExecutionCore::new(provider, registry);

        let mut messages = vec![Message::user("create task: buy")];
        let params = ExecutionParams::new("mock");
        let tools = vec![serde_json::json!({
            "type": "function",
            "function": {
                "name": "todo",
                "description": "Manage tasks",
                "parameters": {"type": "object", "properties": {}}
            }
        })];

        let (outcome, _usage) = core
            .run_cycle(&mut messages, &tools, &params, &routing_ctx())
            .await
            .unwrap();

        assert!(matches!(outcome, CycleOutcome::FinalResponse { .. }));
    }
}
