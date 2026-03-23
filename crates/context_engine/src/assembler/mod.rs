mod cache;
mod types;

pub use types::{AssembledContext, ContextRequest, ExecutionStrategy};

use std::sync::Arc;

use futures_util::future::join_all;
use sha2::{Digest, Sha256};

use providers::Message;
use tokio::sync::Mutex;

use crate::inventory::{ContextInventory, ContextInventoryItem, ContextItemStatus};
use crate::memory_retriever::{MemoryRetriever, MemorySource};
use crate::source::{ContextSource, SourceContext};
use crate::summary_provider::SummaryProvider;
use crate::token_counter::{default_token_counter, TokenCounter};
use common::helpers::tool_def_name;

use crate::{BudgetAllocator, BudgetConfig, CompressorConfig, HistoryCompressor, Priority};

use cache::{ContextCache, DEFAULT_CACHE_CAPACITY};
use types::DEFAULT_MEMORY_RETRIEVAL_LIMIT;

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
    /// Optional InsightForge for multi-dimensional retrieval.
    insight_forge: Option<Arc<crate::insight_forge::InsightForge>>,
    /// Optional query rewriter for contextual query enrichment.
    query_rewriter: Option<Arc<dyn crate::rewriter::QueryRewriter>>,
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
            insight_forge: None,
            query_rewriter: None,
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
            insight_forge: self.insight_forge,
            query_rewriter: self.query_rewriter,
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

    /// Wire a query rewriter for contextual query enrichment.
    /// Returns `self` for chaining.
    pub fn with_query_rewriter(mut self, rewriter: Arc<dyn crate::rewriter::QueryRewriter>) -> Self {
        self.query_rewriter = Some(rewriter);
        self
    }

    /// Wire InsightForge for multi-dimensional context retrieval.
    /// All DomainSearchers must be added to the forge BEFORE calling this,
    /// since it wraps in Arc internally.
    pub fn with_insight_forge(self, forge: crate::insight_forge::InsightForge) -> Self {
        Self {
            insight_forge: Some(Arc::new(forge)),
            ..self
        }
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
            intent_summary: None,
            project_id: None,
        };

        let futures: Vec<_> = self.sources.iter().map(|s| s.provide(&ctx)).collect();
        let results = join_all(futures).await;
        let sections: Vec<_> = results
            .into_iter()
            .flatten()
            .filter(|s| !s.trim().is_empty())
            .collect();

        sections.join("\n\n---\n\n")
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
        if let Some(name) = request.tool_definitions.first().and_then(tool_def_name) {
            hasher.update(name.as_bytes());
        }
        // Hash context window
        hasher.update(request.context_window.to_le_bytes());
        // Hash retrieval context fields that affect cache identity
        if let Some(ref ctx) = request.retrieval_context {
            if let Some(ref skill) = ctx.active_skill {
                hasher.update(skill.as_bytes());
            }
            if let Some(ref task) = ctx.active_task {
                hasher.update(task.title.as_bytes());
            }
            if let Some(ref correction) = ctx.recent_correction {
                hasher.update(correction.corrected_to.as_bytes());
            }
        }
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
        // Skip memory retrieval only for Clarification mode (no RAG needed).
        // DirectResponse still needs retrieval for personalized conversational answers.
        let (memory_content, retrieved_memory_count) = match &request.strategy {
            ExecutionStrategy::Clarification { .. } => (None, 0),
            _ => {
                let result = self.retrieve_memory(request).await;
                match result {
                    Some((text, count)) => (Some(text), count),
                    None => (None, 0),
                }
            }
        };
        let memory_tokens = memory_content
            .as_deref()
            .map(|c| self.estimate_text(c))
            .unwrap_or(0);
        if memory_tokens > 0 {
            allocator.allocate(Priority::RetrievedMemory, memory_tokens);
        }

        // 4. Compress history to fit remaining budget (token-aware)
        let history_budget = allocator.remaining();
        let compressed = self
            .compressor
            .compress_async(&request.history, history_budget)
            .await;

        // Post-compression budget enforcement: if recent messages alone
        // exceed the budget (e.g., very long tool results), truncate from oldest.
        let mut recent_messages = compressed.recent_messages;
        let mut recent_tokens: usize = recent_messages
            .iter()
            .map(|m| self.estimate_message_tokens(m))
            .sum();
        let mut drop_count = 0;
        while recent_tokens > history_budget && (drop_count + 1) < recent_messages.len() {
            let removed_tokens = self.estimate_message_tokens(&recent_messages[drop_count]);
            recent_tokens = recent_tokens.saturating_sub(removed_tokens);
            drop_count += 1;
        }
        if drop_count > 0 {
            recent_messages.drain(..drop_count);
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

        let mut token_count = allocator.total_allocated();
        let mut budget_remaining = allocator.remaining();
        let mut budget_report = allocator.report();

        // Build inventory from registered sources.
        let mut inventory = ContextInventory::new();
        for source in &self.sources {
            inventory.upsert(ContextInventoryItem {
                source_name: source.name().to_string(),
                priority: source.priority(),
                status: ContextItemStatus::Loaded {
                    tokens_used: source.estimated_tokens(),
                },
                token_estimate: source.estimated_tokens(),
                summary: None,
            });
        }

        // Inject inventory summary into the prompt if there are any
        // deferred or available sources the agent could request.
        if inventory.has_deferred() {
            let total_budget = allocator.config_total_window();
            let inventory_text = inventory.format_for_prompt(total_budget, budget_remaining);
            let inv_tokens = self.estimate_text(&inventory_text);
            messages.push(Message::system(&inventory_text));
            token_count += inv_tokens;
            budget_remaining = budget_remaining.saturating_sub(inv_tokens);
            budget_report.remaining = budget_remaining;
        }

        AssembledContext {
            messages,
            token_count,
            budget_report,
            inventory,
            budget_remaining,
            version: 0,
            retrieved_memory_count,
        }
    }

    /// Retrieve relevant memories for the request via embedding-based retrieval.
    /// Returns the formatted memory text and the number of entries retrieved.
    async fn retrieve_memory(&self, request: &ContextRequest) -> Option<(String, usize)> {
        let retriever = self.memory_retriever.as_ref()?;

        let enriched = match (&self.query_rewriter, &request.retrieval_context) {
            (Some(rewriter), Some(ctx)) => rewriter.rewrite(&request.message_text, ctx).await,
            _ => None,
        };
        // Use InsightForge if available and appropriate
        let entries = if let Some(ref forge) = self.insight_forge {
            if forge.should_activate(&request.strategy, &request.message_text) {
                forge
                    .retrieve_with_enrichment(
                        &request.message_text,
                        enriched.as_ref(),
                        self.memory_retrieval_limit,
                        request.session_key.as_deref(),
                    )
                    .await
            } else {
                retriever
                    .retrieve(&request.message_text, self.memory_retrieval_limit)
                    .await
            }
        } else {
            retriever
                .retrieve(&request.message_text, self.memory_retrieval_limit)
                .await
        };
        if entries.is_empty() {
            return None;
        }

        let entry_count = entries.len();

        let (facts, rest): (Vec<_>, Vec<_>) = entries
            .into_iter()
            .partition(|e| matches!(e.source, MemorySource::CognitiveFact));
        let (recalls, domain): (Vec<_>, Vec<_>) = rest
            .into_iter()
            .partition(|e| matches!(e.source, MemorySource::ConversationRecall));

        let mut text = "[Relevant Context]\n".to_string();

        if !facts.is_empty() {
            text.push_str("\n## Relevant Facts\n");
            for entry in &facts {
                text.push_str(&format!(
                    "- {} (relevance: {:.2})\n",
                    entry.content, entry.score
                ));
            }
        }

        if !recalls.is_empty() {
            text.push_str("\n## Related Conversations\n");
            for entry in &recalls {
                text.push_str(&format!(
                    "- {} (relevance: {:.2})\n",
                    entry.content, entry.score
                ));
            }
        }

        if !domain.is_empty() {
            text.push_str("\n## Related Information\n");
            for entry in &domain {
                text.push_str(&format!(
                    "- {} (relevance: {:.2})\n",
                    entry.content, entry.score
                ));
            }
        }

        Some((text, entry_count))
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

    /// Expand context by loading a deferred source.
    ///
    /// Finds the named source, calls `provide()`, and inserts the result
    /// as a system message if it fits within the remaining budget.
    /// Increments the context version on success.
    pub async fn expand(
        &self,
        current: &AssembledContext,
        source_name: &str,
        source_ctx: &SourceContext,
    ) -> common::Result<AssembledContext> {
        let source = self
            .sources
            .iter()
            .find(|s| s.name() == source_name)
            .ok_or_else(|| {
                common::ToolError::ExecutionFailed(format!(
                    "Context source '{}' not found",
                    source_name
                ))
            })?;

        let content = source.provide(source_ctx).await;

        // Pre-compute token cost and check budget before cloning the full context.
        let content_tokens = content.as_ref().map(|text| self.estimate_text(text));
        if let Some(tokens) = content_tokens {
            if tokens > current.budget_remaining {
                return Err(common::ToolError::ExecutionFailed(format!(
                    "Insufficient budget for '{}': needs {} tokens, {} remaining",
                    source_name, tokens, current.budget_remaining
                ))
                .into());
            }
        }

        let mut result = current.clone();
        result.version += 1;

        if let (Some(text), Some(tokens)) = (content, content_tokens) {
            result.messages.push(Message::system(&text));
            result.token_count += tokens;
            result.budget_remaining -= tokens;
            result.budget_report.remaining = result.budget_remaining;
            result.inventory.mark_loaded(source_name, tokens);
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::BudgetReport;
    use crate::inventory::{ContextInventory, ContextInventoryItem, ContextItemStatus};
    use crate::memory_retriever::{MemoryEntry, MemoryRetriever, MemorySource};
    use crate::source::{ContextSource, SourceContext};
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
            session_key: None,
            retrieval_context: None,
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
            session_key: None,
            retrieval_context: None,
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
            session_key: None,
            retrieval_context: None,
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
            session_key: None,
            retrieval_context: None,
        };

        let doubled = engine.assemble(make_req()).await;
        let base = base_engine.assemble(make_req()).await;
        // DoubleCounter should produce exactly twice the token count
        assert_eq!(doubled.token_count, base.token_count * 2);
    }

    // ── G-08: Memory retrieval tests ──

    struct MockRetriever {
        entries: Vec<(String, f64, MemorySource)>,
    }

    #[async_trait]
    impl MemoryRetriever for MockRetriever {
        async fn retrieve(&self, _query: &str, limit: usize) -> Vec<MemoryEntry> {
            self.entries
                .iter()
                .take(limit)
                .map(|(content, score, source)| MemoryEntry {
                    id: "test".into(),
                    content: content.clone(),
                    score: *score,
                    source: source.clone(),
                    raw_score: *score,
                })
                .collect()
        }
    }

    #[tokio::test]
    async fn test_memory_retriever_injects_context() {
        let retriever = Arc::new(MockRetriever {
            entries: vec![
                ("User likes Rust".into(), 0.95, MemorySource::CognitiveFact),
                (
                    "User works on klyntbot".into(),
                    0.80,
                    MemorySource::CognitiveFact,
                ),
            ],
        });

        let engine = ContextEngine::new().with_memory_retriever(retriever);
        let request = ContextRequest {
            message_text: "what do I work on?".into(),
            history: vec![],
            system_prompt: "You are helpful.".into(),
            // Use ToolAssisted — Direct mode skips memory retrieval
            strategy: ExecutionStrategy::ToolAssisted { max_iterations: 5 },
            tool_definitions: vec![],

            context_window: 128_000,
            session_key: None,
            retrieval_context: None,
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
    async fn test_direct_mode_includes_memory_retrieval() {
        let retriever = Arc::new(MockRetriever {
            entries: vec![
                ("User likes Rust".into(), 0.95, MemorySource::CognitiveFact),
                (
                    "User works on klyntbot".into(),
                    0.80,
                    MemorySource::CognitiveFact,
                ),
            ],
        });

        let engine = ContextEngine::new().with_memory_retriever(retriever);
        let request = ContextRequest {
            message_text: "hi there".into(),
            history: vec![],
            system_prompt: "You are helpful.".into(),
            strategy: ExecutionStrategy::DirectResponse,
            tool_definitions: vec![],
            context_window: 128_000,
            session_key: None,
            retrieval_context: None,
        };

        let result = engine.assemble(request).await;

        // Direct mode should include memory for personalized conversational answers
        assert_eq!(result.messages.len(), 2);
        let mem_alloc = result
            .budget_report
            .per_priority
            .iter()
            .find(|(p, _)| *p == Priority::RetrievedMemory);
        assert!(mem_alloc.is_some(), "RetrievedMemory should be allocated");
    }

    #[tokio::test]
    async fn test_clarification_mode_skips_memory_retrieval() {
        let retriever = Arc::new(MockRetriever {
            entries: vec![("Some memory".into(), 0.90, MemorySource::CognitiveFact)],
        });

        let engine = ContextEngine::new().with_memory_retriever(retriever);
        let request = ContextRequest {
            message_text: "what?".into(),
            history: vec![],
            system_prompt: "You are helpful.".into(),
            strategy: ExecutionStrategy::Clarification {
                reason: "Ambiguous request".into(),
            },
            tool_definitions: vec![],
            context_window: 128_000,
            session_key: None,
            retrieval_context: None,
        };

        let result = engine.assemble(request).await;

        // Clarification mode should NOT include memory
        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.retrieved_memory_count, 0);
        assert!(result
            .budget_report
            .per_priority
            .iter()
            .all(|(p, _)| *p != Priority::RetrievedMemory));
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
            session_key: None,
            retrieval_context: None,
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
            session_key: None,
            retrieval_context: None,
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
            session_key: None,
            retrieval_context: None,
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
            async fn summarize_batch(
                &self,
                segments: Vec<Vec<Message>>,
            ) -> Vec<std::result::Result<String, String>> {
                self.called.store(true, std::sync::atomic::Ordering::SeqCst);
                segments
                    .iter()
                    .map(|_| Ok("LLM summary".to_string()))
                    .collect()
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
            session_key: None,
            retrieval_context: None,
        };

        engine.assemble(request).await;

        assert!(
            called.load(std::sync::atomic::Ordering::SeqCst),
            "SummaryProvider should have been called via compress_async"
        );
    }

    // ── Expand tests ──

    struct DeferredSource;

    #[async_trait]
    impl ContextSource for DeferredSource {
        fn name(&self) -> &str {
            "deferred_test"
        }
        fn priority(&self) -> u8 {
            40
        }
        async fn provide(&self, _ctx: &SourceContext) -> Option<String> {
            Some("[Deferred Content]\nProject details here.".into())
        }
        fn estimated_tokens(&self) -> usize {
            200
        }
    }

    fn test_source_context() -> SourceContext {
        SourceContext {
            channel: "cli".into(),
            chat_id: "test".into(),
            message: None,
            intent_summary: None,
            project_id: None,
        }
    }

    #[tokio::test]
    async fn test_expand_loads_deferred_source() {
        let engine = ContextEngine::new().with_sources(vec![Box::new(DeferredSource)]);

        let mut initial = AssembledContext {
            messages: vec![Message::system("System prompt.")],
            token_count: 100,
            budget_report: BudgetReport {
                total_window: 12000,
                total_allocated: 100,
                remaining: 5000,
                per_priority: vec![],
            },
            inventory: ContextInventory::new(),
            budget_remaining: 5000,
            version: 0,
            retrieved_memory_count: 0,
        };
        initial.inventory.upsert(ContextInventoryItem {
            source_name: "deferred_test".into(),
            priority: 40,
            status: ContextItemStatus::Deferred {
                reason: "budget".into(),
            },
            token_estimate: 200,
            summary: None,
        });

        let ctx = test_source_context();
        let expanded = engine
            .expand(&initial, "deferred_test", &ctx)
            .await
            .unwrap();
        assert!(expanded.version > initial.version);
        assert!(expanded.messages.len() > initial.messages.len());
        assert!(!expanded.inventory.has_deferred());
        assert_eq!(expanded.budget_report.remaining, expanded.budget_remaining);
    }

    #[tokio::test]
    async fn test_expand_rejects_unknown_source() {
        let engine = ContextEngine::new();
        let initial = AssembledContext {
            messages: vec![],
            token_count: 0,
            budget_report: BudgetReport {
                total_window: 12000,
                total_allocated: 0,
                remaining: 5000,
                per_priority: vec![],
            },
            inventory: ContextInventory::new(),
            budget_remaining: 5000,
            version: 0,
            retrieved_memory_count: 0,
        };

        let ctx = test_source_context();
        let result = engine.expand(&initial, "nonexistent", &ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_memory_retriever_groups_by_source() {
        let retriever = Arc::new(MockRetriever {
            entries: vec![
                (
                    "user: editor = neovim".into(),
                    0.90,
                    MemorySource::CognitiveFact,
                ),
                (
                    "I mentioned I use neovim yesterday".into(),
                    0.75,
                    MemorySource::ConversationRecall,
                ),
            ],
        });

        let engine = ContextEngine::new().with_memory_retriever(retriever);
        let request = ContextRequest {
            message_text: "what editor do I use?".into(),
            history: vec![],
            system_prompt: "You are helpful.".into(),
            strategy: ExecutionStrategy::ToolAssisted { max_iterations: 5 },
            tool_definitions: vec![],
            context_window: 128_000,
            session_key: None,
            retrieval_context: None,
        };

        let result = engine.assemble(request).await;

        // Verify retrieved_memory_count matches the number of mock entries
        assert_eq!(
            result.retrieved_memory_count, 2,
            "Should report 2 retrieved memory entries"
        );

        if let Message::System { content } = &result.messages[1] {
            assert!(
                content.contains("## Relevant Facts"),
                "Should have facts section"
            );
            assert!(
                content.contains("## Related Conversations"),
                "Should have recalls section"
            );
            // Facts should appear before conversations
            let facts_pos = content.find("## Relevant Facts").unwrap();
            let recalls_pos = content.find("## Related Conversations").unwrap();
            assert!(facts_pos < recalls_pos);
        } else {
            panic!("Second message should be System (memory)");
        }
    }

    #[tokio::test]
    async fn test_expand_rejects_insufficient_budget() {
        let engine = ContextEngine::new().with_sources(vec![Box::new(DeferredSource)]);

        let initial = AssembledContext {
            messages: vec![Message::system("System prompt.")],
            token_count: 100,
            budget_report: BudgetReport {
                total_window: 200,
                total_allocated: 195,
                remaining: 5,
                per_priority: vec![],
            },
            inventory: ContextInventory::new(),
            budget_remaining: 5,
            version: 0,
            retrieved_memory_count: 0,
        };

        let ctx = test_source_context();
        let result = engine.expand(&initial, "deferred_test", &ctx).await;
        assert!(result.is_err());
    }

    fn test_request() -> ContextRequest {
        ContextRequest {
            message_text: "test query".to_string(),
            history: vec![Message::user("hello")],
            system_prompt: "You are helpful.".to_string(),
            strategy: ExecutionStrategy::DirectResponse,
            tool_definitions: vec![],
            context_window: 128_000,
            session_key: None,
            retrieval_context: None,
        }
    }

    #[test]
    fn cache_key_varies_by_skill() {
        let mut req1 = test_request();
        req1.retrieval_context = Some(crate::rewriter::RetrievalContext {
            active_skill: Some("finance-management".into()),
            ..Default::default()
        });
        let mut req2 = test_request();
        req2.retrieval_context = Some(crate::rewriter::RetrievalContext {
            active_skill: Some("task-management".into()),
            ..Default::default()
        });
        assert_ne!(
            ContextEngine::compute_cache_key(&req1),
            ContextEngine::compute_cache_key(&req2),
        );
    }

    #[test]
    fn cache_key_varies_by_task() {
        let mut req1 = test_request();
        req1.retrieval_context = Some(crate::rewriter::RetrievalContext {
            active_task: Some(crate::rewriter::ActiveTaskContext {
                title: "March budget review".into(),
                project_name: None,
                domain: None,
            }),
            ..Default::default()
        });
        let mut req2 = test_request();
        req2.retrieval_context = Some(crate::rewriter::RetrievalContext {
            active_task: Some(crate::rewriter::ActiveTaskContext {
                title: "API migration".into(),
                project_name: None,
                domain: None,
            }),
            ..Default::default()
        });
        assert_ne!(
            ContextEngine::compute_cache_key(&req1),
            ContextEngine::compute_cache_key(&req2),
        );
    }
}
