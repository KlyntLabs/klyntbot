//! MemoryTool - Semantic search over conversation history
//!
//! Provides LLM-accessible actions for searching past conversations using embeddings.
//! Supports both conversation-only search and unified Todo+Conversation search via RRF.

use async_trait::async_trait;
use bus::DomainEventBus;
use context_engine::enhancement::LatestEnhancementTrace;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

use crate::conversation_recall::{ConversationRecallHandler, PurgeFilter, RecallSearchResult};
use crate::embedding_engine::EmbeddingHandler;
use crate::semantic_fact_search::SemanticFactSearchHandler;
use crate::todo_types::Action;
use crate::{approval_class::ApprovalClass, RoutingContext, Tool};
use common::Result;

/// Tool for semantic search over conversation history.
pub struct MemoryTool {
    conversation_handler: Option<Arc<dyn ConversationRecallHandler>>,
    semantic_threshold: f64,
    /// RRF k parameter for hybrid search (from config)
    rrf_k: u32,
    /// Todo repo for unified search (SQL-backed)
    todo_repo: Option<storage::TaskRepo>,
    /// Embedding handler for todo semantic search
    todo_embedding_handler: Option<Arc<dyn EmbeddingHandler>>,
    /// LanceDB vector store for todo semantic search
    embedding_store: Option<storage::VectorStore>,
    /// Domain event bus for publishing facts
    domain_bus: Option<Arc<DomainEventBus>>,
    /// Shared store of the most recent query enhancement trace.
    enhancement_trace: Option<Arc<LatestEnhancementTrace>>,
    /// T2.2: semantic-fact retriever for `search_all`. When wired,
    /// `search_all` queries both conversation embeddings AND the
    /// cognitive semantic-fact pool (same pool as pre-injection).
    fact_search_handler: Option<Arc<dyn SemanticFactSearchHandler>>,
}

impl MemoryTool {
    /// Create a new MemoryTool with default config.
    pub fn new() -> Self {
        Self {
            conversation_handler: None,
            semantic_threshold: 0.5,
            rrf_k: 60,
            todo_repo: None,
            todo_embedding_handler: None,
            embedding_store: None,
            domain_bus: None,
            enhancement_trace: None,
            fact_search_handler: None,
        }
    }

    /// Inject semantic-fact search handler (T2.2).
    pub fn with_fact_search_handler(mut self, handler: Arc<dyn SemanticFactSearchHandler>) -> Self {
        self.fact_search_handler = Some(handler);
        self
    }

    /// Inject conversation recall handler for semantic search.
    pub fn with_conversation_handler(
        mut self,
        handler: Arc<dyn ConversationRecallHandler>,
    ) -> Self {
        self.conversation_handler = Some(handler);
        self
    }

    /// Set semantic search threshold.
    pub fn with_threshold(mut self, threshold: f64) -> Self {
        self.semantic_threshold = threshold;
        self
    }

    /// Set RRF k parameter for unified search.
    pub fn with_rrf_k(mut self, k: u32) -> Self {
        self.rrf_k = k;
        self
    }

    /// Inject todo repo for unified search.
    pub fn with_todo_repo(mut self, repo: storage::TaskRepo) -> Self {
        self.todo_repo = Some(repo);
        self
    }

    /// Inject todo embedding handler for unified search.
    pub fn with_todo_embedding_handler(mut self, handler: Arc<dyn EmbeddingHandler>) -> Self {
        self.todo_embedding_handler = Some(handler);
        self
    }

    /// Inject vector store for todo semantic search.
    pub fn with_embedding_store(mut self, store: storage::VectorStore) -> Self {
        self.embedding_store = Some(store);
        self
    }

    /// Inject domain event bus for publishing facts.
    pub fn with_domain_bus(mut self, bus: Arc<DomainEventBus>) -> Self {
        self.domain_bus = Some(bus);
        self
    }

    /// Inject the shared store that holds the most recent query enhancement
    /// trace. Required for the `get_last_enhancement_trace` action.
    pub fn with_enhancement_trace_store(mut self, store: Arc<LatestEnhancementTrace>) -> Self {
        self.enhancement_trace = Some(store);
        self
    }
}

impl Default for MemoryTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for MemoryTool {
    fn name(&self) -> &str {
        "memory"
    }

    fn description(&self) -> &str {
        "Search past conversations using semantic similarity and record user facts. Actions: search_conversations (search conversation history), search_all (unified search across todos and conversations), purge (clear embeddings), status (show memory stats), record_fact (record a fact about the user into the cognitive pipeline), get_last_enhancement_trace (inspect the most recent query enhancement pipeline run)."
    }

    fn metadata(&self) -> tools_core::ToolMetadata {
        tools_core::ToolMetadata {
            category: tools_core::ToolCategory::Memory,
            tags: vec![
                "memory".into(),
                "fact".into(),
                "recall".into(),
                "learn".into(),
            ],
            cost_hint: tools_core::CostHint::Free,
            ..Default::default()
        }
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "required": ["action"],
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["search_conversations", "search_all", "purge", "status", "record_fact", "get_last_enhancement_trace"],
                    "description": "Action to perform"
                },
                "query": {
                    "type": "string",
                    "description": "Search query (required for search_conversations, search_all)"
                },
                "limit": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 50,
                    "description": "Maximum results to return (default: 10)"
                },
                "threshold": {
                    "type": "number",
                    "minimum": 0.0,
                    "maximum": 1.0,
                    "description": "Cosine similarity threshold (default: 0.5)"
                },
                "widen": {
                    "type": "boolean",
                    "description": "When true, search_all probes the cognitive fact pool with broader thresholds (lowered similarity, doubled limits). Use this on retrieval-empty refusals."
                },
                "filter": {
                    "type": "string",
                    "enum": ["all", "session", "before_date"],
                    "description": "Purge filter (for purge action)"
                },
                "session_key": {
                    "type": "string",
                    "description": "Session key for session filter (purge action)"
                },
                "before_date": {
                    "type": "string",
                    "description": "ISO 8601 date for before_date filter (purge action)"
                },
                "fact": {
                    "type": "string",
                    "description": "A fact about the user to remember (required for record_fact)"
                },
                "domain": {
                    "type": "string",
                    "enum": ["identity", "energy", "work", "finance", "learning", "preferences", "general"],
                    "description": "Domain category for the fact (required for record_fact)"
                }
            }
        })
    }

    async fn execute(&self, args: Value, _ctx: &RoutingContext) -> Result<String> {
        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| common::ToolError::InvalidParams("action required".to_string()))?;

        match action {
            "search_conversations" => self.search_conversations(&args).await,
            "search_all" => self.search_all(&args).await,
            "purge" => self.purge_embeddings(&args).await,
            "status" => self.show_status().await,
            "record_fact" => self.record_fact(&args).await,
            "get_last_enhancement_trace" => self.get_last_enhancement_trace().await,
            _ => Err(common::KlyntbotError::Tool(
                common::ToolError::InvalidParams(format!("Unknown action: {}", action)),
            )),
        }
    }

    fn approval_class(&self, args: &Value) -> ApprovalClass {
        match args.get("action").and_then(|v| v.as_str()) {
            Some("record_fact") => ApprovalClass::Sensitive,
            Some("purge") => ApprovalClass::Destructive,
            _ => ApprovalClass::Safe,
        }
    }
}

impl MemoryTool {
    /// Search conversations using semantic similarity.
    async fn search_conversations(&self, args: &Value) -> Result<String> {
        let handler = self.conversation_handler.as_ref().ok_or_else(|| {
            common::ToolError::InvalidParams(
                "Conversation embedding handler not available".to_string(),
            )
        })?;

        if !handler.is_available() {
            return Ok(
                "Semantic search is not available (embedding model not loaded).".to_string(),
            );
        }

        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| common::ToolError::InvalidParams("query required".to_string()))?;

        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

        let threshold = args
            .get("threshold")
            .and_then(|v| v.as_f64())
            .unwrap_or(self.semantic_threshold);

        let results: Vec<RecallSearchResult> = handler.search(query, limit, threshold).await?;

        if results.is_empty() {
            return Ok(format!(
                "No conversations found matching '{}' (threshold: {:.2}).",
                query, threshold
            ));
        }

        let mut output = format!("{} conversation(s) matching '{}':\n", results.len(), query);

        for result in &results {
            output.push_str(&format!(
                "\n- [{}] {} said: \"{}\" (similarity: {:.3}, session: {})",
                result.id, result.role, result.content_preview, result.score, result.session_key
            ));
        }

        Ok(output)
    }

    /// Unified search across todos and conversations using RRF.
    async fn search_all(&self, args: &Value) -> Result<String> {
        let handler = self.conversation_handler.as_ref().ok_or_else(|| {
            common::ToolError::InvalidParams(
                "Conversation embedding handler not available".to_string(),
            )
        })?;

        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| common::ToolError::InvalidParams("query required".to_string()))?;

        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

        let threshold = args
            .get("threshold")
            .and_then(|v| v.as_f64())
            .unwrap_or(self.semantic_threshold);

        if !handler.is_available() {
            return Ok(
                "Semantic search is not available (embedding model not loaded).".to_string(),
            );
        }

        // T2.2 — query semantic-fact pool ONLY when the caller
        // requests widened search (typically the Tier 2 refusal-retry
        // path). Default search_all calls stay on conversation+todo
        // index to avoid drowning the model in widened-pool noise.
        // The bench measured -26pp regression when this fired on
        // every search_all call.
        let widen = args.get("widen").and_then(|v| v.as_bool()).unwrap_or(false);
        let mut facts_block = String::new();
        if widen {
            if let Some(fh) = &self.fact_search_handler {
                let fact_limit = limit.max(20);
                match fh.search_facts(query, fact_limit, true).await {
                    Ok(facts) if !facts.is_empty() => {
                        facts_block.push_str("## Semantic Facts\n");
                        for f in &facts {
                            facts_block.push_str(&format!(
                                "- [{:.2}] {}: {} = {} ({})\n",
                                f.score, f.subject, f.predicate, f.object, f.domain
                            ));
                        }
                        facts_block.push('\n');
                    }
                    Ok(_) => {}
                    Err(e) => {
                        tracing::warn!(error = %e, "fact search failed in search_all");
                    }
                }
            }
        }

        // 1. Search conversations (service already overfetches internally for decay filtering)
        let conversation_results: Vec<RecallSearchResult> =
            handler.search(query, limit, threshold).await?;

        // 2. Search todos (keyword + semantic if available)
        let todo_results: Vec<(Action, f64)> = if let Some(todo_repo) = &self.todo_repo {
            // Keyword search via SQL ILIKE (escapes special chars)
            let keyword_rows = todo_repo.search_by_keyword(query, None).await?;
            let keyword_todos: Vec<Action> = keyword_rows.into_iter().map(Action::from).collect();

            // Semantic search via LanceDB (if available)
            let semantic_todos: Vec<(String, f64)> = if let (Some(emb), Some(vs)) =
                (&self.todo_embedding_handler, &self.embedding_store)
            {
                let query_vec = emb.embed_query(query).await?;
                vs.search_similar("todo_embeddings", &query_vec, 100, threshold)
                    .await
                    .unwrap_or_default()
            } else {
                Vec::new()
            };

            // Merge keyword and semantic results using RRF
            if !semantic_todos.is_empty() {
                // Build lookup from keyword results
                let mut todos_by_id: HashMap<String, crate::search_utils::SearchResult> =
                    keyword_todos
                        .iter()
                        .map(|t| {
                            (
                                t.id.clone(),
                                crate::search_utils::SearchResult::Todo(Box::new(t.clone())),
                            )
                        })
                        .collect();

                // Fetch any semantic-only todos not already in keyword results
                for (id, _) in &semantic_todos {
                    if !todos_by_id.contains_key(id) {
                        if let Ok(Some(row)) = todo_repo.get(id).await {
                            let todo = Action::from(row);
                            todos_by_id.insert(
                                id.clone(),
                                crate::search_utils::SearchResult::Todo(Box::new(todo)),
                            );
                        }
                    }
                }

                let keyword_search_results: Vec<crate::search_utils::SearchResult> = keyword_todos
                    .into_iter()
                    .map(|t| crate::search_utils::SearchResult::Todo(Box::new(t)))
                    .collect();

                let merged = crate::search_utils::rrf_merge(
                    &keyword_search_results,
                    &semantic_todos,
                    self.rrf_k,
                    &todos_by_id,
                );

                merged
                    .into_iter()
                    .filter_map(|(result, score, _source)| match result {
                        crate::search_utils::SearchResult::Todo(todo) => Some((*todo, score)),
                        crate::search_utils::SearchResult::Conversation(_) => None,
                    })
                    .collect()
            } else {
                // Keyword-only results with simple scoring
                keyword_todos
                    .into_iter()
                    .enumerate()
                    .map(|(rank, todo)| {
                        let score = 1.0 / (self.rrf_k as f64 + rank as f64 + 1.0);
                        (todo, score)
                    })
                    .collect()
            }
        } else {
            Vec::new()
        };

        // 3. Convert to SearchResult and merge using RRF
        let conversation_search_results: Vec<crate::search_utils::SearchResult> =
            conversation_results
                .iter()
                .map(|r| crate::search_utils::SearchResult::Conversation(r.clone()))
                .collect();

        let todo_search_results: Vec<crate::search_utils::SearchResult> = todo_results
            .iter()
            .map(|(todo, _)| crate::search_utils::SearchResult::Todo(Box::new(todo.clone())))
            .collect();

        // Build ID lookup map for conversations
        let mut results_by_id: HashMap<String, crate::search_utils::SearchResult> = HashMap::new();
        for r in &conversation_results {
            results_by_id.insert(
                r.id.clone(),
                crate::search_utils::SearchResult::Conversation(r.clone()),
            );
        }
        for (todo, _) in &todo_results {
            results_by_id.insert(
                todo.id.clone(),
                crate::search_utils::SearchResult::Todo(Box::new(todo.clone())),
            );
        }

        // Prepare semantic results for RRF (both conversations and todos)
        let mut semantic_results: Vec<(String, f64)> = conversation_results
            .iter()
            .map(|r| (r.id.clone(), r.score))
            .collect();
        semantic_results.extend(
            todo_results
                .iter()
                .map(|(todo, score)| (todo.id.clone(), *score)),
        );

        // Merge all results using RRF
        let all_keyword_results: Vec<crate::search_utils::SearchResult> =
            conversation_search_results
                .into_iter()
                .chain(todo_search_results.into_iter())
                .collect();

        let merged = crate::search_utils::rrf_merge(
            &all_keyword_results,
            &semantic_results,
            self.rrf_k,
            &results_by_id,
        );

        // 4. Format output
        if merged.is_empty() {
            // Even when no conv/todo hits, return facts_block if present.
            // Prepend a one-line preamble so the model has an anchor describing
            // what the bullet list represents (otherwise it tends to produce
            // empty output when faced with an unframed list of triples).
            if !facts_block.is_empty() {
                return Ok(format!(
                    "Found {} fact(s) for '{}' from semantic memory (no conversation/todo matches):\n\n{}",
                    facts_block.matches("\n- ").count(),
                    query,
                    facts_block
                ));
            }
            return Ok(format!(
                "No results found matching '{}' (threshold: {:.2}).",
                query, threshold
            ));
        }

        let mut output = facts_block.clone();
        output.push_str(&format!(
            "{} result(s) matching '{}' (unified search: todos + conversations):\n",
            merged.len().min(limit),
            query
        ));

        for (result, rrf_score, source) in merged.iter().take(limit) {
            match result {
                crate::search_utils::SearchResult::Conversation(record) => {
                    output.push_str(&format!(
                        "\n- [Conversation|{}] {} said: \"{}\" (score: {:.4}, session: {})",
                        source, record.role, record.content_preview, rrf_score, record.session_key
                    ));
                }
                crate::search_utils::SearchResult::Todo(todo) => {
                    let preview = if let Some(desc) = &todo.description {
                        format!(
                            "{} - {}",
                            todo.title,
                            &desc.chars().take(50).collect::<String>()
                        )
                    } else {
                        todo.title.clone()
                    };
                    output.push_str(&format!(
                        "\n- [Todo|{}] {} (score: {:.4}, status: {:?})",
                        source, preview, rrf_score, todo.status
                    ));
                }
            }
        }

        Ok(output)
    }

    /// Purge conversation embeddings.
    async fn purge_embeddings(&self, args: &Value) -> Result<String> {
        let handler = self.conversation_handler.as_ref().ok_or_else(|| {
            common::ToolError::InvalidParams(
                "Conversation embedding handler not available".to_string(),
            )
        })?;

        // SAFETY: Require explicit filter to prevent accidental data loss
        let filter_type = args
            .get("filter")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                common::ToolError::InvalidParams(
                    "filter parameter is required for purge action (use 'all', 'session', or 'before_date')".to_string()
                )
            })?;

        let filter = match filter_type {
            "session" => {
                let session_key = args
                    .get("session_key")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        common::ToolError::InvalidParams(
                            "session_key required for session filter".to_string(),
                        )
                    })?;
                PurgeFilter::BySessionKey(session_key.to_string())
            }
            "before_date" => {
                let date_str = args
                    .get("before_date")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        common::ToolError::InvalidParams(
                            "before_date required for before_date filter".to_string(),
                        )
                    })?;
                let date = date_str.parse::<jiff::Timestamp>().map_err(|e| {
                    common::ToolError::InvalidParams(format!("Invalid date: {}", e))
                })?;
                PurgeFilter::Before(date)
            }
            "all" => PurgeFilter::All,
            _ => {
                return Err(common::KlyntbotError::Tool(
                    common::ToolError::InvalidParams(format!(
                        "Unknown filter type: {}",
                        filter_type
                    )),
                ))
            }
        };

        let deleted_count = handler.purge(filter).await?;

        Ok(format!(
            "Purged {} conversation embedding(s).",
            deleted_count
        ))
    }

    /// Record a user-stated fact and publish it as a domain event.
    async fn record_fact(&self, args: &Value) -> Result<String> {
        let fact = args
            .get("fact")
            .and_then(|v| v.as_str())
            .ok_or_else(|| common::ToolError::InvalidParams("fact required".to_string()))?;

        let domain = args
            .get("domain")
            .and_then(|v| v.as_str())
            .unwrap_or("general");

        let bus = self.domain_bus.as_ref().ok_or_else(|| {
            common::ToolError::InvalidParams("Fact recording not available".to_string())
        })?;

        bus.publish(bus::DomainEvent::UserStatedFact {
            fact: fact.to_string(),
            domain: domain.to_string(),
        });

        Ok(format!("Recorded: \"{fact}\" (domain: {domain})"))
    }

    /// Return a JSON-encoded snapshot of the most recent query enhancement
    /// pipeline run, or a notice if none has been captured yet.
    async fn get_last_enhancement_trace(&self) -> Result<String> {
        let store = self.enhancement_trace.as_ref().ok_or_else(|| {
            common::ToolError::InvalidParams("Enhancement trace store not configured".to_string())
        })?;

        match store.get() {
            Some(snapshot) => serde_json::to_string_pretty(&snapshot).map_err(|e| {
                common::KlyntbotError::Tool(common::ToolError::InvalidParams(format!(
                    "failed to serialize enhancement trace: {e}"
                )))
            }),
            None => Ok("No enhancement trace captured yet.".to_string()),
        }
    }

    /// Show conversation recall status.
    async fn show_status(&self) -> Result<String> {
        let handler = self.conversation_handler.as_ref().ok_or_else(|| {
            common::ToolError::InvalidParams(
                "Conversation recall handler not available".to_string(),
            )
        })?;

        let status = handler.status().await?;

        let mut output = String::from("Conversation Memory Status:\n");
        output.push_str(&format!("\nTotal embeddings: {}", status.total_embeddings));
        output.push_str(&format!(
            "\nAvailable: {}",
            if status.is_available { "yes" } else { "no" }
        ));

        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversation_recall::ConversationRecallStatus;

    /// Mock handler for testing
    struct MockConversationHandler {
        available: bool,
    }

    #[async_trait]
    impl ConversationRecallHandler for MockConversationHandler {
        async fn embed_message(
            &self,
            _session_key: &str,
            _role: &str,
            _content: &str,
            _message_id: &str,
        ) -> Result<()> {
            Ok(())
        }

        async fn search(
            &self,
            query: &str,
            limit: usize,
            _threshold: f64,
        ) -> Result<Vec<RecallSearchResult>> {
            if query == "test query" {
                let result = RecallSearchResult {
                    id: "msg-1".to_string(),
                    session_key: "cli:test".to_string(),
                    role: "user".to_string(),
                    content_preview: "This is a test message".to_string(),
                    content_full: "This is a test message".to_string(),
                    score: 0.85,
                    created_at: jiff::Timestamp::now(),
                };
                Ok(vec![result].into_iter().take(limit).collect())
            } else {
                Ok(vec![])
            }
        }

        async fn purge(&self, filter: PurgeFilter) -> Result<usize> {
            match filter {
                PurgeFilter::All => Ok(10),
                PurgeFilter::BySessionKey(_) => Ok(3),
                PurgeFilter::Before(_) => Ok(5),
            }
        }

        async fn status(&self) -> Result<ConversationRecallStatus> {
            Ok(ConversationRecallStatus {
                total_embeddings: 42,
                is_available: self.available,
            })
        }

        fn is_available(&self) -> bool {
            self.available
        }
    }

    #[test]
    fn test_tool_trait_implementation() {
        let tool = MemoryTool::new();

        assert_eq!(tool.name(), "memory");
        assert!(tool.description().contains("Search past conversations"));

        let params = tool.parameters();
        assert_eq!(params["required"], json!(["action"]));
        assert_eq!(
            params["properties"]["action"]["enum"],
            json!([
                "search_conversations",
                "search_all",
                "purge",
                "status",
                "record_fact",
                "get_last_enhancement_trace"
            ])
        );
    }

    #[test]
    fn test_action_routing() {
        let tool = MemoryTool::new();
        let params = tool.parameters();

        let actions = params["properties"]["action"]["enum"].as_array().unwrap();
        assert_eq!(actions.len(), 6);
        assert!(actions.contains(&json!("search_conversations")));
        assert!(actions.contains(&json!("search_all")));
        assert!(actions.contains(&json!("purge")));
        assert!(actions.contains(&json!("status")));
        assert!(actions.contains(&json!("record_fact")));
        assert!(actions.contains(&json!("get_last_enhancement_trace")));
    }

    #[tokio::test]
    async fn get_last_enhancement_trace_returns_snapshot() {
        use context_engine::enhancement::{
            EnhancementTrace, EnhancementTraceSnapshot, LatestEnhancementTrace, QueryBundle,
            QuerySource,
        };

        let store = Arc::new(LatestEnhancementTrace::new());
        let mut bundle = QueryBundle::passthrough("what's my next task?");
        bundle.primary = "what's my next task? (skill: tasks)".to_string();
        bundle.variants = vec!["upcoming todos".to_string()];
        bundle.confidence = 0.75;
        bundle.sources.push(QuerySource::SignalEnrichment);

        store.set(EnhancementTraceSnapshot {
            query: bundle,
            trace: EnhancementTrace::default(),
            captured_at: jiff::Timestamp::now(),
        });

        let tool = MemoryTool::new().with_enhancement_trace_store(Arc::clone(&store));
        let ctx = RoutingContext::new(
            common::ChannelName::new("cli"),
            common::ChatId::new("test".to_string()),
        );

        let result = tool
            .execute(json!({"action": "get_last_enhancement_trace"}), &ctx)
            .await
            .unwrap();

        assert!(result.contains("what's my next task?"));
        assert!(result.contains("upcoming todos"));
        assert!(result.contains("confidence"));
    }

    #[tokio::test]
    async fn get_last_enhancement_trace_when_empty() {
        use context_engine::enhancement::LatestEnhancementTrace;

        let store = Arc::new(LatestEnhancementTrace::new());
        let tool = MemoryTool::new().with_enhancement_trace_store(store);
        let ctx = RoutingContext::new(
            common::ChannelName::new("cli"),
            common::ChatId::new("test".to_string()),
        );

        let result = tool
            .execute(json!({"action": "get_last_enhancement_trace"}), &ctx)
            .await
            .unwrap();

        assert!(result.contains("No enhancement trace captured yet"));
    }

    #[tokio::test]
    async fn get_last_enhancement_trace_requires_store() {
        let tool = MemoryTool::new();
        let ctx = RoutingContext::new(
            common::ChannelName::new("cli"),
            common::ChatId::new("test".to_string()),
        );

        let result = tool
            .execute(json!({"action": "get_last_enhancement_trace"}), &ctx)
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_search_conversations_action_params() {
        let mock_handler = Arc::new(MockConversationHandler { available: true });
        let tool = MemoryTool::new().with_conversation_handler(mock_handler);

        let ctx = RoutingContext::new(
            common::ChannelName::new("cli"),
            common::ChatId::new("test".to_string()),
        );

        // Test with valid params
        let args = json!({
            "action": "search_conversations",
            "query": "test query",
            "limit": 5,
            "threshold": 0.7
        });

        let result = tool.execute(args, &ctx).await.unwrap();
        assert!(result.contains("1 conversation(s) matching"));
        assert!(result.contains("msg-1"));
        assert!(result.contains("similarity: 0.850"));

        // Test missing query
        let args = json!({
            "action": "search_conversations"
        });

        let result = tool.execute(args, &ctx).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("query required"));
    }

    #[tokio::test]
    async fn test_purge_action_params() {
        let mock_handler = Arc::new(MockConversationHandler { available: true });
        let tool = MemoryTool::new().with_conversation_handler(mock_handler);

        let ctx = RoutingContext::new(
            common::ChannelName::new("cli"),
            common::ChatId::new("test".to_string()),
        );

        // Test purge all
        let args = json!({
            "action": "purge",
            "filter": "all"
        });

        let result = tool.execute(args, &ctx).await.unwrap();
        assert!(result.contains("Purged 10 conversation embedding(s)"));

        // Test purge by session
        let args = json!({
            "action": "purge",
            "filter": "session",
            "session_key": "cli:test"
        });

        let result = tool.execute(args, &ctx).await.unwrap();
        assert!(result.contains("Purged 3 conversation embedding(s)"));

        // Test purge before date
        let args = json!({
            "action": "purge",
            "filter": "before_date",
            "before_date": "2024-01-01T00:00:00Z"
        });

        let result = tool.execute(args, &ctx).await.unwrap();
        assert!(result.contains("Purged 5 conversation embedding(s)"));
    }

    #[tokio::test]
    async fn test_status_action() {
        let mock_handler = Arc::new(MockConversationHandler { available: true });
        let tool = MemoryTool::new().with_conversation_handler(mock_handler);

        let ctx = RoutingContext::new(
            common::ChannelName::new("cli"),
            common::ChatId::new("test".to_string()),
        );

        let args = json!({
            "action": "status"
        });

        let result = tool.execute(args, &ctx).await.unwrap();
        assert!(result.contains("Total embeddings: 42"));
        assert!(result.contains("Available: yes"));
    }

    #[tokio::test]
    async fn test_handler_not_available() {
        let mock_handler = Arc::new(MockConversationHandler { available: false });
        let tool = MemoryTool::new().with_conversation_handler(mock_handler);

        let ctx = RoutingContext::new(
            common::ChannelName::new("cli"),
            common::ChatId::new("test".to_string()),
        );

        let args = json!({
            "action": "search_conversations",
            "query": "test"
        });

        let result = tool.execute(args, &ctx).await.unwrap();
        assert!(result.contains("Semantic search is not available"));
    }

    #[tokio::test]
    async fn test_no_results() {
        let mock_handler = Arc::new(MockConversationHandler { available: true });
        let tool = MemoryTool::new().with_conversation_handler(mock_handler);

        let ctx = RoutingContext::new(
            common::ChannelName::new("cli"),
            common::ChatId::new("test".to_string()),
        );

        let args = json!({
            "action": "search_conversations",
            "query": "no results query"
        });

        let result = tool.execute(args, &ctx).await.unwrap();
        assert!(result.contains("No conversations found matching"));
    }

    #[tokio::test]
    async fn record_fact_publishes_event() {
        let bus = Arc::new(DomainEventBus::new(16));
        let mut rx = bus.subscribe();
        let tool = MemoryTool::new().with_domain_bus(Arc::clone(&bus));
        let args = json!({
            "action": "record_fact",
            "fact": "User is a software engineer",
            "domain": "identity"
        });
        let ctx = RoutingContext::new(
            common::ChannelName::new("cli"),
            common::ChatId::new("test".to_string()),
        );
        let result = tool.execute(args, &ctx).await.unwrap();
        assert!(result.contains("Recorded"));

        let event = rx.try_recv().unwrap();
        match event {
            bus::DomainEvent::UserStatedFact { fact, domain } => {
                assert_eq!(fact, "User is a software engineer");
                assert_eq!(domain, "identity");
            }
            other => panic!("Expected UserStatedFact, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn record_fact_requires_bus() {
        let tool = MemoryTool::new();
        let args = json!({"action": "record_fact", "fact": "x", "domain": "general"});
        let ctx = RoutingContext::new(
            common::ChannelName::new("cli"),
            common::ChatId::new("test".to_string()),
        );
        assert!(tool.execute(args, &ctx).await.is_err());
    }

    #[tokio::test]
    async fn record_fact_requires_fact_param() {
        let bus = Arc::new(DomainEventBus::new(16));
        let tool = MemoryTool::new().with_domain_bus(bus);
        let args = json!({"action": "record_fact", "domain": "general"});
        let ctx = RoutingContext::new(
            common::ChannelName::new("cli"),
            common::ChatId::new("test".to_string()),
        );
        assert!(tool.execute(args, &ctx).await.is_err());
    }
}
