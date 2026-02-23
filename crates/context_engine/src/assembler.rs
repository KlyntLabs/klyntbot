use std::collections::HashMap;
use std::sync::Arc;

use sha2::{Digest, Sha256};

use providers::Message;
use tokio::sync::Mutex;

use crate::memory_retriever::MemoryRetriever;
use crate::source::{ContextSource, SourceContext};
use crate::summary_provider::SummaryProvider;
use crate::token_counter::{default_token_counter, TokenCounter};
use crate::{
    BudgetAllocator, BudgetConfig, BudgetReport, CompressorConfig, HistoryCompressor, Priority,
};

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
    /// Model context window size (varies per model).
    pub context_window: usize,
}

/// The assembled context ready to send to the LLM.
#[derive(Clone)]
pub struct AssembledContext {
    /// Ordered messages: system, memories, summaries, recent history.
    pub messages: Vec<Message>,
    /// Estimated total token count.
    pub token_count: usize,
    /// Budget allocation report.
    pub budget_report: BudgetReport,
}

/// Default number of memory entries to retrieve.
const DEFAULT_MEMORY_RETRIEVAL_LIMIT: usize = 5;

/// Maximum number of entries in the context assembly cache.
const DEFAULT_CACHE_CAPACITY: usize = 8;

/// Bounded cache for assembled contexts, keyed by a hash of the request inputs.
struct ContextCache {
    entries: HashMap<String, AssembledContext>,
    /// Insertion order for eviction (oldest first).
    order: Vec<String>,
    capacity: usize,
    /// Generation counter — incremented on invalidation.
    generation: u64,
    /// Generation at which each entry was inserted.
    entry_generations: HashMap<String, u64>,
}

impl ContextCache {
    fn new(capacity: usize) -> Self {
        Self {
            entries: HashMap::with_capacity(capacity),
            order: Vec::with_capacity(capacity),
            capacity,
            generation: 0,
            entry_generations: HashMap::with_capacity(capacity),
        }
    }

    fn get(&self, key: &str) -> Option<&AssembledContext> {
        // Only return if entry is from the current generation
        let entry_gen = self.entry_generations.get(key)?;
        if *entry_gen < self.generation {
            return None;
        }
        self.entries.get(key)
    }

    fn insert(&mut self, key: String, value: AssembledContext) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            // Evict oldest
            if let Some(oldest_key) = self.order.first().cloned() {
                self.entries.remove(&oldest_key);
                self.entry_generations.remove(&oldest_key);
                self.order.remove(0);
            }
        }
        if !self.order.contains(&key) {
            self.order.push(key.clone());
        }
        self.entries.insert(key.clone(), value);
        self.entry_generations.insert(key, self.generation);
    }

    fn invalidate(&mut self) {
        self.generation += 1;
    }
}

/// Orchestrates budget allocation, history compression, memory retrieval,
/// and system prompt assembly via pluggable context sources.
pub struct ContextEngine {
    compressor: HistoryCompressor,
    token_counter: Arc<dyn TokenCounter>,
    memory_retriever: Option<Arc<dyn MemoryRetriever>>,
    /// Maximum number of memory entries to retrieve per query.
    memory_retrieval_limit: usize,
    /// Cache for assembled contexts.
    cache: Arc<Mutex<ContextCache>>,
    /// Pluggable context sources for system prompt assembly, sorted by priority (descending).
    sources: Vec<Box<dyn ContextSource>>,
}

impl Default for ContextEngine {
    fn default() -> Self {
        let counter = default_token_counter();
        let config = CompressorConfig::default();
        Self {
            compressor: HistoryCompressor::from_config(Arc::clone(&counter), config),
            token_counter: counter,
            memory_retriever: None,
            memory_retrieval_limit: DEFAULT_MEMORY_RETRIEVAL_LIMIT,
            cache: Arc::new(Mutex::new(ContextCache::new(DEFAULT_CACHE_CAPACITY))),
            sources: Vec::new(),
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
        let config = CompressorConfig::default();
        Self {
            compressor: HistoryCompressor::from_config(Arc::clone(&counter), config),
            token_counter: counter,
            memory_retriever: self.memory_retriever,
            memory_retrieval_limit: self.memory_retrieval_limit,
            cache: self.cache,
            sources: self.sources,
        }
    }

    /// Override the compressor configuration.
    /// Returns `self` for chaining.
    pub fn with_compressor_config(self, config: CompressorConfig) -> Self {
        Self {
            compressor: HistoryCompressor::from_config(Arc::clone(&self.token_counter), config),
            ..self
        }
    }

    /// Set the maximum number of memory entries to retrieve per query.
    /// Returns `self` for chaining.
    pub fn with_memory_retrieval_limit(self, limit: usize) -> Self {
        Self {
            memory_retrieval_limit: limit,
            ..self
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

    /// Set a `SummaryProvider` for abstractive history compression.
    /// Returns `self` for chaining.
    pub fn with_summary_provider(mut self, provider: Arc<dyn SummaryProvider>) -> Self {
        self.compressor = self.compressor.with_summary_provider(provider);
        self
    }

    /// Register pluggable context sources for system prompt assembly.
    ///
    /// Sources are sorted by priority (descending) so higher-priority
    /// sections appear first in the assembled system prompt.
    /// Returns `self` for chaining.
    pub fn with_sources(mut self, mut sources: Vec<Box<dyn ContextSource>>) -> Self {
        sources.sort_by_key(|s| std::cmp::Reverse(s.priority()));
        self.sources = sources;
        self
    }

    /// Build the system prompt by iterating registered context sources.
    ///
    /// Each source is queried for its section; non-empty sections are
    /// joined with `\n\n---\n\n` separators — matching the separator
    /// format previously used by `ContextBuilder`.
    ///
    /// Returns an empty string if no sources are registered.
    pub async fn build_system_prompt(
        &self,
        channel: &str,
        chat_id: &str,
        message: Option<&str>,
    ) -> String {
        if self.sources.is_empty() {
            return String::new();
        }

        let ctx = SourceContext {
            channel: channel.to_string(),
            chat_id: chat_id.to_string(),
            message: message.map(|s| s.to_string()),
        };

        let mut sections = Vec::with_capacity(self.sources.len());
        for source in &self.sources {
            if let Some(section) = source.provide(&ctx).await {
                if !section.trim().is_empty() {
                    sections.push(section);
                }
            }
        }

        sections.join("\n\n---\n\n")
    }

    /// Invalidate the assembled context cache.
    ///
    /// Call this after tool executions or config changes that affect
    /// the context assembly output.
    pub async fn invalidate_cache(&self) {
        self.cache.lock().await.invalidate();
    }

    pub async fn assemble(&self, request: ContextRequest) -> AssembledContext {
        // Check cache first
        let cache_key = Self::compute_cache_key(&request);
        {
            let cache = self.cache.lock().await;
            if let Some(cached) = cache.get(&cache_key) {
                return cached.clone();
            }
        }

        let result = self.assemble_uncached(&request).await;

        // Store in cache
        {
            let mut cache = self.cache.lock().await;
            cache.insert(cache_key, result.clone());
        }

        result
    }

    /// Compute a stable, deterministic cache key from the request inputs.
    ///
    /// Uses SHA-256 to produce a 64-char hex string that is stable across
    /// process restarts (unlike `DefaultHasher` which is randomized).
    fn compute_cache_key(request: &ContextRequest) -> String {
        let mut hasher = Sha256::new();
        // Hash system prompt
        hasher.update(request.system_prompt.as_bytes());
        // Hash history length + last message content (changes on each user message)
        hasher.update(request.history.len().to_le_bytes());
        if let Some(last) = request.history.last() {
            hasher.update(format!("{:?}", last).as_bytes());
        }
        // Hash message text (used for memory retrieval)
        hasher.update(request.message_text.as_bytes());
        // Hash strategy discriminant as a single byte for determinism
        let strategy_byte: u8 = match &request.strategy {
            ExecutionStrategy::DirectResponse => 0,
            ExecutionStrategy::ToolAssisted { .. } => 1,
            ExecutionStrategy::AutonomousTask { .. } => 2,
            ExecutionStrategy::Clarification { .. } => 3,
        };
        hasher.update([strategy_byte]);
        // Hash tool definition count + first tool name (lightweight proxy)
        hasher.update(request.tool_definitions.len().to_le_bytes());
        if let Some(first) = request.tool_definitions.first() {
            if let Some(name) = first
                .get("function")
                .and_then(|f| f.get("name"))
                .and_then(|n| n.as_str())
            {
                hasher.update(name.as_bytes());
            }
        }
        // Hash context window
        hasher.update(request.context_window.to_le_bytes());
        format!("{:x}", hasher.finalize())
    }

    async fn assemble_uncached(&self, request: &ContextRequest) -> AssembledContext {
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
        let memory_content = self.retrieve_memory(request).await;
        let memory_tokens = memory_content
            .as_deref()
            .map(|c| self.estimate_text(c))
            .unwrap_or(0);
        if memory_tokens > 0 {
            allocator.allocate(Priority::RetrievedMemory, memory_tokens);
        }

        // 4. Compress history to fit remaining budget (token-aware)
        let history_budget = allocator.remaining();
        let compressed = self.compressor.compress(&request.history, history_budget);

        // Post-compression budget enforcement: if recent messages alone
        // exceed the budget (e.g., very long tool results), truncate from oldest.
        let mut recent_messages = compressed.recent_messages;
        let mut recent_tokens: usize = recent_messages
            .iter()
            .map(|m| self.estimate_message_tokens(m))
            .sum();
        while recent_tokens > history_budget && recent_messages.len() > 1 {
            let removed_tokens = self.estimate_message_tokens(&recent_messages[0]);
            recent_messages.remove(0);
            recent_tokens = recent_tokens.saturating_sub(removed_tokens);
        }

        // Track actual allocations
        allocator.allocate(Priority::RecentHistory, recent_tokens);

        let remaining_after_recent = history_budget.saturating_sub(recent_tokens);
        let summaries: Vec<_> = compressed
            .summaries
            .into_iter()
            .scan(0usize, |acc, s| {
                *acc += s.token_count;
                if *acc <= remaining_after_recent {
                    Some(s)
                } else {
                    None
                }
            })
            .collect();
        let summary_tokens: usize = summaries.iter().map(|s| s.token_count).sum();
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
        for summary in &summaries {
            messages.push(Message::system(&summary.content));
        }

        // Recent messages verbatim
        messages.extend(recent_messages);

        let token_count = allocator.total_allocated();
        let budget_report = allocator.report();

        AssembledContext {
            messages,
            token_count,
            budget_report,
        }
    }

    /// Retrieve relevant memories for the request via embedding-based retrieval.
    async fn retrieve_memory(&self, request: &ContextRequest) -> Option<String> {
        if let Some(retriever) = &self.memory_retriever {
            let entries = retriever
                .retrieve(&request.message_text, self.memory_retrieval_limit)
                .await;
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

    #[test]
    fn test_cache_key_is_deterministic() {
        let req = ContextRequest {
            system_prompt: "You are helpful.".to_string(),
            history: vec![Message::user("hello")],
            message_text: "test".to_string(),
            strategy: ExecutionStrategy::ToolAssisted { max_iterations: 5 },
            tool_definitions: vec![],
            context_window: 4096,
        };
        let key1 = ContextEngine::compute_cache_key(&req);
        let key2 = ContextEngine::compute_cache_key(&req);
        assert_eq!(key1, key2, "Cache key must be deterministic");
        // SHA-256 produces a 64-char hex string
        assert_eq!(key1.len(), 64, "Expected SHA-256 hex string (64 chars)");
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
    async fn test_token_budget_truncates_long_messages() {
        let engine = ContextEngine::new();
        // Create history with very long messages that would blow a small budget
        let mut history = Vec::new();
        for i in 0..10 {
            // Each message is ~250 tokens (1000 chars / 4 chars per token)
            let long_text = format!("Message {} {}", i, "x".repeat(1000));
            if i % 2 == 0 {
                history.push(Message::user(long_text));
            } else {
                history.push(Message::assistant(long_text));
            }
        }

        let request = ContextRequest {
            message_text: "test".to_string(),
            history,
            system_prompt: "System.".to_string(),
            strategy: ExecutionStrategy::DirectResponse,
            tool_definitions: vec![],
            context_window: 1000, // very small window — ~850 input budget
        };
        let result = engine.assemble(request).await;

        // Token count must stay within 85% of context_window
        assert!(
            result.token_count <= 850,
            "Token count {} should not exceed input budget 850",
            result.token_count
        );
        // Should still have at least the system message
        assert!(!result.messages.is_empty());
    }

    #[tokio::test]
    async fn test_abstractive_compression_used_when_provider_wired() {
        use crate::history_compressor::CompressorMode;
        use crate::summary_provider::SummaryProvider;
        use crate::CompressorConfig;

        struct TrackingProvider {
            called: std::sync::Arc<std::sync::atomic::AtomicBool>,
        }

        #[async_trait]
        impl SummaryProvider for TrackingProvider {
            async fn summarize(
                &self,
                _messages: &[Message],
            ) -> std::result::Result<String, String> {
                self.called.store(true, std::sync::atomic::Ordering::SeqCst);
                Ok("LLM summary".to_string())
            }
        }

        let called = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let provider = Arc::new(TrackingProvider {
            called: called.clone(),
        });

        let config = CompressorConfig {
            mode: CompressorMode::Abstractive,
            min_recent_messages: 2,
            chunk_size: 3,
            ..Default::default()
        };

        let engine = ContextEngine::new()
            .with_compressor_config(config)
            .with_summary_provider(provider);

        // 20 messages to ensure some get compressed
        let mut history = Vec::new();
        for i in 0..20 {
            if i % 2 == 0 {
                history.push(Message::user(format!("User message {}", i)));
            } else {
                history.push(Message::assistant(format!("Response {}", i)));
            }
        }

        let request = ContextRequest {
            message_text: "test".to_string(),
            history,
            system_prompt: "System.".to_string(),
            strategy: ExecutionStrategy::DirectResponse,
            tool_definitions: vec![],
            // Small enough to force compression: available_input ≈ 43,
            // system ≈ 2 tokens, history_budget ≈ 41, which is less than
            // all 20 messages (~70 tokens total).
            context_window: 50,
        };

        engine.assemble(request).await;

        assert!(
            called.load(std::sync::atomic::Ordering::SeqCst),
            "SummaryProvider should have been called via compress_async"
        );
    }
}
