use std::path::PathBuf;

use providers::Message;

use crate::{BudgetAllocator, BudgetConfig, BudgetReport, HistoryCompressor, Priority};

/// Determines how the agent should process a request.
#[derive(Debug, Clone)]
pub enum ExecutionStrategy {
    /// Simple question/answer — no tool use needed.
    DirectResponse,
    /// May use tools up to `max_iterations` rounds.
    ToolAssisted { max_iterations: u32 },
    /// Full autonomous multi-step execution.
    AutonomousTask { max_iterations: u32 },
    /// Need more info from the user before proceeding.
    Clarification { reason: String },
}

/// Input to the context assembly pipeline.
pub struct ContextRequest {
    /// The user's message text (used for embedding-based memory lookup).
    pub message_text: String,
    /// Full conversation history.
    pub history: Vec<Message>,
    /// System prompt to prepend.
    pub system_prompt: String,
    /// Chosen execution strategy (affects budget allocation).
    pub strategy: ExecutionStrategy,
    /// Tool definitions as JSON schemas.
    pub tool_definitions: Vec<serde_json::Value>,
    /// Path to MEMORY.md for retrieval (optional).
    pub memory_path: Option<PathBuf>,
    /// Model context window size (varies per model).
    pub context_window: usize,
}

/// The assembled context ready to send to the LLM.
pub struct AssembledContext {
    /// Ordered messages: system, summaries, recent history.
    pub messages: Vec<Message>,
    /// Estimated total token count.
    pub token_count: usize,
    /// Budget allocation report.
    pub budget_report: BudgetReport,
}

/// Orchestrates budget allocation, history compression, and memory retrieval
/// into a single assembled context that fits within the model's context window.
pub struct ContextEngine {
    compressor: HistoryCompressor,
}

impl Default for ContextEngine {
    fn default() -> Self {
        Self {
            compressor: HistoryCompressor::new(4),
        }
    }
}

impl ContextEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn assemble(&self, request: ContextRequest) -> AssembledContext {
        let mut allocator = BudgetAllocator::new(BudgetConfig::standard(request.context_window));

        // 1. System prompt always gets allocated first
        let system_tokens = estimate_tokens(&request.system_prompt);
        allocator.allocate(Priority::SystemIdentity, system_tokens);

        // 2. Tool definitions budget depends on strategy
        let tool_tokens = match &request.strategy {
            ExecutionStrategy::DirectResponse | ExecutionStrategy::Clarification { .. } => 0,
            ExecutionStrategy::ToolAssisted { .. } => {
                estimate_tool_tokens(&request.tool_definitions)
            }
            ExecutionStrategy::AutonomousTask { .. } => {
                estimate_tool_tokens(&request.tool_definitions)
            }
        };
        allocator.allocate(Priority::ToolDefinitions, tool_tokens);

        // 3. Compress history to fit remaining budget
        // Split remaining between recent history and compressed history
        let history_budget = allocator.remaining();
        let recent_budget = history_budget * 60 / 100; // 60% for recent
        let compressed_budget = history_budget * 40 / 100; // 40% for compressed/summaries

        let compressed = self
            .compressor
            .compress(&request.history, recent_budget + compressed_budget);

        // Track actual allocations
        let recent_tokens: usize = compressed
            .recent_messages
            .iter()
            .map(estimate_message_tokens)
            .sum();
        allocator.allocate(Priority::RecentHistory, recent_tokens);

        let summary_tokens: usize = compressed.summaries.iter().map(|s| s.token_count).sum();
        allocator.allocate(Priority::CompressedHistory, summary_tokens);

        // 4. Build the final message list
        let mut messages = Vec::new();

        // System message
        messages.push(Message::system(&request.system_prompt));

        // Summaries as system-level context (if any)
        for summary in &compressed.summaries {
            messages.push(Message::system(&summary.content));
        }

        // Recent messages verbatim
        messages.extend(compressed.recent_messages);

        let token_count = allocator.total_allocated();
        let budget_report = allocator.report();

        AssembledContext {
            messages,
            token_count,
            budget_report,
        }
    }
}

fn estimate_tokens(text: &str) -> usize {
    text.len().div_ceil(4)
}

fn estimate_message_tokens(msg: &Message) -> usize {
    match msg {
        Message::System { content } => estimate_tokens(content),
        Message::User { content } => match content {
            providers::UserContent::Text(t) => estimate_tokens(t),
            providers::UserContent::MultiPart(parts) => parts.len() * 10,
        },
        Message::Assistant { content, .. } => {
            content.as_deref().map(estimate_tokens).unwrap_or(0) + 20
        }
        Message::Tool { content, .. } => estimate_tokens(content) + 10,
    }
}

fn estimate_tool_tokens(tools: &[serde_json::Value]) -> usize {
    tools
        .iter()
        .map(|t| {
            let s = t.to_string();
            estimate_tokens(&s)
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_request(
        strategy: ExecutionStrategy,
        tool_count: usize,
        context_window: usize,
    ) -> ContextRequest {
        let mut history = Vec::new();
        for i in 0..10 {
            if i % 2 == 0 {
                history.push(Message::user(format!("User message {}", i)));
            } else {
                history.push(Message::assistant(format!("Assistant response {}", i)));
            }
        }

        let tool_definitions: Vec<serde_json::Value> = (0..tool_count)
            .map(|i| {
                serde_json::json!({
                    "name": format!("tool_{}", i),
                    "description": format!("A test tool number {}", i),
                    "parameters": { "type": "object", "properties": {} }
                })
            })
            .collect();

        ContextRequest {
            message_text: "test query".to_string(),
            history,
            system_prompt: "You are a helpful assistant.".to_string(),
            strategy,
            tool_definitions,
            memory_path: None,
            context_window,
        }
    }

    #[test]
    fn test_direct_response_minimal_context() {
        let engine = ContextEngine::new();
        let request = make_request(ExecutionStrategy::DirectResponse, 5, 128_000);
        let result = engine.assemble(request);

        // Should have system message + history messages
        assert!(!result.messages.is_empty());
        // First message should be the system prompt
        if let Message::System { content } = &result.messages[0] {
            assert!(content.contains("helpful assistant"));
        } else {
            panic!("First message should be System");
        }
        // Tool definitions should NOT be counted in budget (DirectResponse = 0 tools budget)
        let tool_alloc = result
            .budget_report
            .per_priority
            .iter()
            .find(|(p, _)| *p == Priority::ToolDefinitions);
        assert!(
            tool_alloc.is_none() || tool_alloc.unwrap().1 == 0,
            "DirectResponse should have no tool budget"
        );
    }

    #[test]
    fn test_tool_assisted_includes_tools() {
        let engine = ContextEngine::new();
        let request = make_request(
            ExecutionStrategy::ToolAssisted { max_iterations: 5 },
            3,
            128_000,
        );
        let result = engine.assemble(request);

        // Tool definitions should be allocated budget
        let tool_alloc = result
            .budget_report
            .per_priority
            .iter()
            .find(|(p, _)| *p == Priority::ToolDefinitions);
        assert!(
            tool_alloc.is_some(),
            "ToolAssisted should allocate tool budget"
        );
        assert!(tool_alloc.unwrap().1 > 0, "Tool budget should be non-zero");
    }

    #[test]
    fn test_context_fits_within_window() {
        let window = 4_000; // small window
        let engine = ContextEngine::new();
        let request = make_request(
            ExecutionStrategy::ToolAssisted { max_iterations: 3 },
            2,
            window,
        );
        let result = engine.assemble(request);

        // Token count should not exceed the input budget (85% of window)
        let input_budget = (window as f32 * 0.85) as usize;
        assert!(
            result.token_count <= input_budget,
            "Token count {} should not exceed input budget {}",
            result.token_count,
            input_budget
        );
    }

    #[test]
    fn test_empty_history_assembles() {
        let engine = ContextEngine::new();
        let request = ContextRequest {
            message_text: "hello".to_string(),
            history: vec![],
            system_prompt: "System prompt.".to_string(),
            strategy: ExecutionStrategy::DirectResponse,
            tool_definitions: vec![],
            memory_path: None,
            context_window: 128_000,
        };
        let result = engine.assemble(request);
        // Should have at least the system message
        assert_eq!(result.messages.len(), 1);
        assert!(result.token_count > 0);
    }

    #[test]
    fn test_clarification_strategy_no_tools() {
        let engine = ContextEngine::new();
        let request = make_request(
            ExecutionStrategy::Clarification {
                reason: "Ambiguous request".to_string(),
            },
            5,
            128_000,
        );
        let result = engine.assemble(request);

        let tool_alloc = result
            .budget_report
            .per_priority
            .iter()
            .find(|(p, _)| *p == Priority::ToolDefinitions);
        assert!(
            tool_alloc.is_none() || tool_alloc.unwrap().1 == 0,
            "Clarification should have no tool budget"
        );
    }
}
