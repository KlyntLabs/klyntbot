use std::path::PathBuf;
use std::sync::Arc;

use providers::Message;

use crate::memory_retriever::MemoryRetriever;
use crate::token_counter::{default_token_counter, TokenCounter};
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
    /// Path to MEMORY.md for retrieval (optional). Used when no `MemoryRetriever` is wired in.
    pub memory_path: Option<PathBuf>,
    /// Model context window size (varies per model).
    pub context_window: usize,
}

/// The assembled context ready to send to the LLM.
pub struct AssembledContext {
    /// Ordered messages: system, memories, summaries, recent history.
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
    token_counter: Arc<dyn TokenCounter>,
    memory_retriever: Option<Arc<dyn MemoryRetriever>>,
}

impl Default for ContextEngine {
    fn default() -> Self {
        let counter = default_token_counter();
        Self {
            compressor: HistoryCompressor::new(4, Arc::clone(&counter)),
            token_counter: counter,
            memory_retriever: None,
        }
    }
}

impl ContextEngine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Override the token counter (e.g., with a provider-specific estimator).
    /// Returns `self` for chaining.
    pub fn with_token_counter(self, counter: Arc<dyn TokenCounter>) -> Self {
        Self {
            compressor: HistoryCompressor::new(4, Arc::clone(&counter)),
            token_counter: counter,
            memory_retriever: self.memory_retriever,
        }
    }

    /// Wire in an optional memory retriever for embedding-based context augmentation.
    /// Returns `self` for chaining.
    pub fn with_memory_retriever(self, retriever: Arc<dyn MemoryRetriever>) -> Self {
        Self {
            memory_retriever: Some(retriever),
            ..self
        }
    }

    pub async fn assemble(&self, request: ContextRequest) -> AssembledContext {
        let mut allocator = BudgetAllocator::new(BudgetConfig::standard(request.context_window));

        // 1. System prompt always gets allocated first
        let system_tokens = self.estimate_text(&request.system_prompt);
        allocator.allocate(Priority::SystemIdentity, system_tokens);

        // 2. Tool definitions budget depends on strategy
        let tool_tokens = match &request.strategy {
            ExecutionStrategy::DirectResponse | ExecutionStrategy::Clarification { .. } => 0,
            ExecutionStrategy::ToolAssisted { .. } | ExecutionStrategy::AutonomousTask { .. } => {
                self.estimate_tool_tokens(&request.tool_definitions)
            }
        };
        allocator.allocate(Priority::ToolDefinitions, tool_tokens);

        // 3. Retrieve memories and allocate budget (Priority::RetrievedMemory)
        let memory_content = self.retrieve_memory(&request).await;
        let memory_tokens = memory_content
            .as_deref()
            .map(|c| self.estimate_text(c))
            .unwrap_or(0);
        if memory_tokens > 0 {
            allocator.allocate(Priority::RetrievedMemory, memory_tokens);
        }

        // 4. Compress history to fit remaining budget
        let history_budget = allocator.remaining();
        let compressed = self
            .compressor
            .compress(&request.history, history_budget);

        // Track actual allocations
        let recent_tokens: usize = compressed
            .recent_messages
            .iter()
            .map(|m| self.estimate_message_tokens(m))
            .sum();
        allocator.allocate(Priority::RecentHistory, recent_tokens);

        let summary_tokens: usize = compressed.summaries.iter().map(|s| s.token_count).sum();
        allocator.allocate(Priority::CompressedHistory, summary_tokens);

        // 5. Build the final message list
        let mut messages = Vec::new();

        // System message
        messages.push(Message::system(&request.system_prompt));

        // Retrieved memory context (if any), injected as a system message
        if let Some(mem_text) = &memory_content {
            messages.push(Message::system(mem_text));
        }

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

    /// Retrieve relevant memories for the request.
    ///
    /// Preference order:
    /// 1. `MemoryRetriever` (embedding-based, injected via `with_memory_retriever`)
    /// 2. `memory_path` from `ContextRequest` (file-based MEMORY.md fallback)
    async fn retrieve_memory(&self, request: &ContextRequest) -> Option<String> {
        // Embedding-based retrieval takes precedence
        if let Some(retriever) = &self.memory_retriever {
            let entries = retriever.retrieve(&request.message_text, 5).await;
            if !entries.is_empty() {
                let mut text = "[Relevant Context]\n".to_string();
                for entry in entries {
                    text.push_str(&format!(
                        "- {} (relevance: {:.2})\n",
                        entry.content, entry.score
                    ));
                }
                return Some(text);
            }
        }

        // File-based fallback (MEMORY.md)
        if let Some(path) = &request.memory_path {
            match tokio::fs::read_to_string(path).await {
                Ok(content) if !content.trim().is_empty() => return Some(content),
                _ => {}
            }
        }

        None
    }

    fn estimate_text(&self, text: &str) -> usize {
        self.token_counter.estimate_text(text)
    }

    fn estimate_message_tokens(&self, msg: &Message) -> usize {
        match msg {
            Message::System { content } => self.estimate_text(content),
            Message::User { content } => match content {
                providers::UserContent::Text(t) => self.estimate_text(t),
                providers::UserContent::MultiPart(parts) => parts.len() * 10,
            },
            Message::Assistant { content, .. } => {
                content
                    .as_deref()
                    .map(|t| self.estimate_text(t))
                    .unwrap_or(0)
                    + 20
            }
            Message::Tool { content, .. } => self.estimate_text(content) + 10,
        }
    }

    fn estimate_tool_tokens(&self, tools: &[serde_json::Value]) -> usize {
        tools
            .iter()
            .map(|t| self.estimate_text(&t.to_string()))
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory_retriever::{MemoryEntry, MemoryRetriever};
    use async_trait::async_trait;
    use tempfile::NamedTempFile;

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

    #[tokio::test]
    async fn test_direct_response_minimal_context() {
        let engine = ContextEngine::new();
        let request = make_request(ExecutionStrategy::DirectResponse, 5, 128_000);
        let result = engine.assemble(request).await;

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

    #[tokio::test]
    async fn test_tool_assisted_includes_tools() {
        let engine = ContextEngine::new();
        let request = make_request(
            ExecutionStrategy::ToolAssisted { max_iterations: 5 },
            3,
            128_000,
        );
        let result = engine.assemble(request).await;

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

    #[tokio::test]
    async fn test_context_fits_within_window() {
        let window = 4_000; // small window
        let engine = ContextEngine::new();
        let request = make_request(
            ExecutionStrategy::ToolAssisted { max_iterations: 3 },
            2,
            window,
        );
        let result = engine.assemble(request).await;

        // Token count should not exceed the input budget (85% of window)
        let input_budget = (window as f32 * 0.85) as usize;
        assert!(
            result.token_count <= input_budget,
            "Token count {} should not exceed input budget {}",
            result.token_count,
            input_budget
        );
    }

    #[tokio::test]
    async fn test_empty_history_assembles() {
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
        let result = engine.assemble(request).await;
        // Should have at least the system message
        assert_eq!(result.messages.len(), 1);
        assert!(result.token_count > 0);
    }

    #[tokio::test]
    async fn test_clarification_strategy_no_tools() {
        let engine = ContextEngine::new();
        let request = make_request(
            ExecutionStrategy::Clarification {
                reason: "Ambiguous request".to_string(),
            },
            5,
            128_000,
        );
        let result = engine.assemble(request).await;

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

    // ── G-07: Token counter tests ──

    #[tokio::test]
    async fn test_custom_token_counter_wired() {
        use crate::token_counter::TokenCounter;

        // A counter that returns 0 for all text-based estimation
        struct ZeroCounter;
        impl TokenCounter for ZeroCounter {
            fn estimate_text(&self, _text: &str) -> usize {
                0
            }
        }

        let engine = ContextEngine::new().with_token_counter(Arc::new(ZeroCounter));
        // Use only user messages — assistant messages add a fixed +20 overhead
        // that is per-message (not text-based) and thus not affected by the counter.
        let request = ContextRequest {
            message_text: "hello".to_string(),
            history: vec![Message::user("hi"), Message::user("there")],
            system_prompt: "You are helpful.".to_string(),
            strategy: ExecutionStrategy::DirectResponse,
            tool_definitions: vec![],
            memory_path: None,
            context_window: 128_000,
        };
        let result = engine.assemble(request).await;
        // ZeroCounter returns 0 for all text → token_count for user + system messages = 0
        assert_eq!(result.token_count, 0);
    }

    #[tokio::test]
    async fn test_with_token_counter_builder() {
        use crate::token_counter::{CharTokenCounter, TokenCounter};

        // Doubled counter
        struct DoubleCounter;
        impl TokenCounter for DoubleCounter {
            fn estimate_text(&self, text: &str) -> usize {
                CharTokenCounter.estimate_text(text) * 2
            }
        }

        let engine = ContextEngine::new().with_token_counter(Arc::new(DoubleCounter));
        let base_engine = ContextEngine::new();

        let make_req = || ContextRequest {
            message_text: "test".to_string(),
            history: vec![],
            system_prompt: "System.".to_string(),
            strategy: ExecutionStrategy::DirectResponse,
            tool_definitions: vec![],
            memory_path: None,
            context_window: 128_000,
        };

        let doubled = engine.assemble(make_req()).await;
        let base = base_engine.assemble(make_req()).await;
        // DoubleCounter should produce exactly twice the token count
        assert_eq!(doubled.token_count, base.token_count * 2);
    }

    // ── G-08: Memory retrieval tests ──

    struct MockRetriever {
        entries: Vec<(String, f64)>,
    }

    #[async_trait]
    impl MemoryRetriever for MockRetriever {
        async fn retrieve(&self, _query: &str, limit: usize) -> Vec<MemoryEntry> {
            self.entries
                .iter()
                .take(limit)
                .map(|(content, score)| MemoryEntry {
                    id: "test".into(),
                    content: content.clone(),
                    score: *score,
                })
                .collect()
        }
    }

    #[tokio::test]
    async fn test_memory_retriever_injects_context() {
        let retriever = Arc::new(MockRetriever {
            entries: vec![
                ("User likes Rust".into(), 0.95),
                ("User works on klyntbot".into(), 0.80),
            ],
        });

        let engine = ContextEngine::new().with_memory_retriever(retriever);
        let request = ContextRequest {
            message_text: "what do I work on?".into(),
            history: vec![],
            system_prompt: "You are helpful.".into(),
            strategy: ExecutionStrategy::DirectResponse,
            tool_definitions: vec![],
            memory_path: None,
            context_window: 128_000,
        };

        let result = engine.assemble(request).await;

        // Memory should appear as the second system message
        assert!(result.messages.len() >= 2);
        if let Message::System { content } = &result.messages[1] {
            assert!(
                content.contains("Relevant Context"),
                "Memory message should contain [Relevant Context]"
            );
            assert!(content.contains("User likes Rust"));
            assert!(content.contains("User works on klyntbot"));
        } else {
            panic!("Second message should be System (memory)");
        }

        // RetrievedMemory should be in the budget report
        let mem_alloc = result
            .budget_report
            .per_priority
            .iter()
            .find(|(p, _)| *p == Priority::RetrievedMemory);
        assert!(mem_alloc.is_some(), "RetrievedMemory should be allocated");
        assert!(mem_alloc.unwrap().1 > 0);
    }

    #[tokio::test]
    async fn test_empty_memory_retriever_no_extra_message() {
        let retriever = Arc::new(MockRetriever { entries: vec![] });
        let engine = ContextEngine::new().with_memory_retriever(retriever);

        let request = ContextRequest {
            message_text: "hello".into(),
            history: vec![],
            system_prompt: "You are helpful.".into(),
            strategy: ExecutionStrategy::DirectResponse,
            tool_definitions: vec![],
            memory_path: None,
            context_window: 128_000,
        };

        let result = engine.assemble(request).await;
        // No memory entries → only the system prompt
        assert_eq!(result.messages.len(), 1);
        assert!(result
            .budget_report
            .per_priority
            .iter()
            .all(|(p, _)| *p != Priority::RetrievedMemory));
    }

    #[tokio::test]
    async fn test_memory_path_fallback() {
        use std::io::Write;
        let mut tmp = NamedTempFile::new().unwrap();
        writeln!(tmp, "# Memory\n- I prefer dark mode").unwrap();

        let engine = ContextEngine::new(); // no MemoryRetriever wired
        let request = ContextRequest {
            message_text: "preferences?".into(),
            history: vec![],
            system_prompt: "You are helpful.".into(),
            strategy: ExecutionStrategy::DirectResponse,
            tool_definitions: vec![],
            memory_path: Some(tmp.path().to_path_buf()),
            context_window: 128_000,
        };

        let result = engine.assemble(request).await;
        // Memory file content injected as second system message
        assert!(result.messages.len() >= 2);
        if let Message::System { content } = &result.messages[1] {
            assert!(content.contains("dark mode"));
        } else {
            panic!("Second message should be memory content");
        }
    }

    #[tokio::test]
    async fn test_memory_retriever_takes_precedence_over_path() {
        use std::io::Write;
        let mut tmp = NamedTempFile::new().unwrap();
        writeln!(tmp, "# Memory\n- file content").unwrap();

        let retriever = Arc::new(MockRetriever {
            entries: vec![("embedding content".into(), 0.9)],
        });
        let engine = ContextEngine::new().with_memory_retriever(retriever);

        let request = ContextRequest {
            message_text: "query".into(),
            history: vec![],
            system_prompt: "You are helpful.".into(),
            strategy: ExecutionStrategy::DirectResponse,
            tool_definitions: vec![],
            memory_path: Some(tmp.path().to_path_buf()),
            context_window: 128_000,
        };

        let result = engine.assemble(request).await;
        // Embedding retriever result should win
        if let Message::System { content } = &result.messages[1] {
            assert!(
                content.contains("embedding content"),
                "MemoryRetriever should take precedence over memory_path"
            );
            assert!(!content.contains("file content"));
        } else {
            panic!("Second message should be System");
        }
    }
}
