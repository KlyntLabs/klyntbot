//! MemoryTool - Semantic search over conversation history
//!
//! Provides LLM-accessible actions for searching past conversations using embeddings.
//! Supports both conversation-only search and unified Todo+Conversation search via RRF.

use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::conversation_embedding::{ConversationEmbeddingHandler, ConversationEmbeddingRecord, PurgeFilter};
use crate::{RoutingContext, Tool};
use common::Result;

/// Tool for semantic search over conversation history.
pub struct MemoryTool {
    conversation_handler: Option<Arc<dyn ConversationEmbeddingHandler>>,
    semantic_threshold: f64,
}

impl MemoryTool {
    /// Create a new MemoryTool with default config.
    pub fn new() -> Self {
        Self {
            conversation_handler: None,
            semantic_threshold: 0.5,
        }
    }

    /// Inject conversation embedding handler for semantic search.
    pub fn with_conversation_handler(
        mut self,
        handler: Arc<dyn ConversationEmbeddingHandler>,
    ) -> Self {
        self.conversation_handler = Some(handler);
        self
    }

    /// Set semantic search threshold.
    pub fn with_threshold(mut self, threshold: f64) -> Self {
        self.semantic_threshold = threshold;
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
        "Search past conversations using semantic similarity. Actions: search_conversations (search conversation history), search_all (unified search across todos and conversations), purge (clear embeddings), status (show memory stats)."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "required": ["action"],
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["search_conversations", "search_all", "purge", "status"],
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
            _ => Err(common::KlyntbotError::Tool(common::ToolError::InvalidParams(format!(
                "Unknown action: {}",
                action
            )))),
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
            return Ok("Semantic search is not available (embedding model not loaded).".to_string());
        }

        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| common::ToolError::InvalidParams("query required".to_string()))?;

        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as usize;

        let threshold = args
            .get("threshold")
            .and_then(|v| v.as_f64())
            .unwrap_or(self.semantic_threshold);

        let results: Vec<(ConversationEmbeddingRecord, f64)> = handler.search(query, limit, threshold).await?;

        if results.is_empty() {
            return Ok(format!(
                "No conversations found matching '{}' (threshold: {:.2}).",
                query, threshold
            ));
        }

        let mut output = format!(
            "{} conversation(s) matching '{}':\n",
            results.len(),
            query
        );

        for (record, similarity) in &results {
            output.push_str(&format!(
                "\n- [{}] {} said: \"{}\" (similarity: {:.3}, session: {})",
                record.id,
                record.role,
                record.content_preview,
                similarity,
                record.session_key
            ));
        }

        Ok(output)
    }

    /// Unified search across todos and conversations using RRF.
    /// Phase 4.1: Conversation-only implementation (TodoTool integration in Phase 4).
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

        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as usize;

        let threshold = args
            .get("threshold")
            .and_then(|v| v.as_f64())
            .unwrap_or(self.semantic_threshold);

        // Phase 4.1: Conversation-only search (TodoTool integration in Phase 4)
        // For now, unified search = conversation search only

        if !handler.is_available() {
            return Ok("Semantic search is not available (embedding model not loaded).".to_string());
        }

        // Search conversations
        let conversation_results: Vec<(ConversationEmbeddingRecord, f64)> = handler.search(query, limit, threshold).await?;

        // TODO: In Phase 4, add TodoTool integration here:
        // let todo_results = self.todo_handler.search(query, limit)?;
        // let merged = rrf_merge(todo_results, conversation_results, 60);

        // For now, return conversation results only
        if conversation_results.is_empty() {
            return Ok(format!(
                "No results found matching '{}' (threshold: {:.2}).",
                query, threshold
            ));
        }

        let mut output = format!(
            "{} result(s) matching '{}':\n",
            conversation_results.len(),
            query
        );

        for (record, similarity) in &conversation_results {
            output.push_str(&format!(
                "\n- [Conversation] {} said: \"{}\" (similarity: {:.3}, session: {})",
                record.role,
                record.content_preview,
                similarity,
                record.session_key
            ));
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
                let session_key = args.get("session_key").and_then(|v| v.as_str()).ok_or_else(|| {
                    common::ToolError::InvalidParams("session_key required for session filter".to_string())
                })?;
                PurgeFilter::BySessionKey(session_key.to_string())
            }
            "before_date" => {
                let date_str = args.get("before_date").and_then(|v| v.as_str()).ok_or_else(|| {
                    common::ToolError::InvalidParams("before_date required for before_date filter".to_string())
                })?;
                let date = chrono::DateTime::parse_from_rfc3339(date_str)
                    .map_err(|e| common::ToolError::InvalidParams(format!("Invalid date: {}", e)))?
                    .with_timezone(&chrono::Utc);
                PurgeFilter::Before(date)
            }
            "all" => PurgeFilter::All,
            _ => {
                return Err(common::KlyntbotError::Tool(common::ToolError::InvalidParams(format!(
                    "Unknown filter type: {}",
                    filter_type
                ))))
            }
        };

        let deleted_count = handler.purge(filter).await?;

        Ok(format!(
            "Purged {} conversation embedding(s).",
            deleted_count
        ))
    }

    /// Show conversation embedding store status.
    async fn show_status(&self) -> Result<String> {
        let handler = self.conversation_handler.as_ref().ok_or_else(|| {
            common::ToolError::InvalidParams(
                "Conversation embedding handler not available".to_string(),
            )
        })?;

        let status = handler.status().await?;

        let mut output = String::from("Conversation Memory Status:\n");
        output.push_str(&format!("\nTotal embeddings: {}", status.total_embeddings));
        output.push_str(&format!(
            "\nAvailable: {}",
            if status.is_available { "yes" } else { "no" }
        ));

        if !status.indexed_channels.is_empty() {
            output.push_str(&format!(
                "\nIndexed channels: {}",
                status.indexed_channels.join(", ")
            ));
        }

        if let Some(oldest) = status.oldest_embedding {
            output.push_str(&format!("\nOldest: {}", oldest.format("%Y-%m-%d %H:%M")));
        }

        if let Some(newest) = status.newest_embedding {
            output.push_str(&format!("\nNewest: {}", newest.format("%Y-%m-%d %H:%M")));
        }

        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversation_embedding::{ConversationEmbeddingRecord, ConversationEmbeddingStatus};
    use chrono::Utc;

    // ──────────────────────────────────────────────────────────────────────
    // Section 3.3: MemoryTool Unit Tests
    // ──────────────────────────────────────────────────────────────────────

    /// Mock handler for testing
    struct MockConversationHandler {
        available: bool,
    }

    #[async_trait]
    impl ConversationEmbeddingHandler for MockConversationHandler {
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
        ) -> Result<Vec<(ConversationEmbeddingRecord, f64)>> {
            // Return mock results
            if query == "test query" {
                let record = ConversationEmbeddingRecord {
                    id: "msg-1".to_string(),
                    session_key: "cli:test".to_string(),
                    role: "user".to_string(),
                    content_preview: "This is a test message".to_string(),
                    embedding: vec![0.1; 384],
                    model: "test-model".to_string(),
                    embedded_at: Utc::now(),
                };
                Ok(vec![(record, 0.85)].into_iter().take(limit).collect())
            } else {
                Ok(vec![])
            }
        }

        async fn purge(&self, filter: PurgeFilter) -> Result<usize> {
            // Return mock count based on filter
            match filter {
                PurgeFilter::All => Ok(10),
                PurgeFilter::BySessionKey(_) => Ok(3),
                PurgeFilter::Before(_) => Ok(5),
            }
        }

        async fn status(&self) -> Result<ConversationEmbeddingStatus> {
            Ok(ConversationEmbeddingStatus {
                total_embeddings: 42,
                indexed_channels: vec!["cli:test".to_string(), "telegram:123".to_string()],
                oldest_embedding: Some(Utc::now() - chrono::Duration::days(30)),
                newest_embedding: Some(Utc::now()),
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
            json!(["search_conversations", "search_all", "purge", "status"])
        );
    }

    #[test]
    fn test_action_routing() {
        let tool = MemoryTool::new();
        let params = tool.parameters();

        // Verify all 4 actions are in the enum
        let actions = params["properties"]["action"]["enum"].as_array().unwrap();
        assert_eq!(actions.len(), 4);
        assert!(actions.contains(&json!("search_conversations")));
        assert!(actions.contains(&json!("search_all")));
        assert!(actions.contains(&json!("purge")));
        assert!(actions.contains(&json!("status")));
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
        assert!(result.contains("Indexed channels: cli:test, telegram:123"));
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
}
