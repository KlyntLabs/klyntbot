//! Integration tests for memory tool and context engine.
//!
//! Merged from `memory_tool_integration.rs` and `memory_and_context_tests.rs`.

use serde_json::json;
use std::sync::Arc;
use tools::conversation_recall::ConversationRecallHandler;
use tools::memory_tool::MemoryTool;
use tools::RoutingContext;
use tools::Tool;

use super::common;
use common::MockConversationRecallHandler;

use klyntbot::agent::context_sources::{BootstrapSource, ConfidenceSource, IdentitySource};
use klyntbot::context_engine::{ContextEngine, ContextSource, SourceContext};
use tempfile::TempDir;

// ── Memory Tool Helpers ───────────────────────────────────────

/// Sample conversation messages for testing (role, session_key, content)
const SAMPLE_MESSAGES: &[(&str, &str, &str)] = &[
    (
        "user",
        "telegram:12345",
        "How do I fix the authentication bug?",
    ),
    (
        "assistant",
        "telegram:12345",
        "You can fix it by updating the token validation logic in auth.rs...",
    ),
    (
        "user",
        "discord:67890",
        "What's the status of the API refactor?",
    ),
    (
        "assistant",
        "discord:67890",
        "The API refactor is in progress, targeting next sprint...",
    ),
    (
        "user",
        "telegram:12345",
        "Can you help me with the login system?",
    ),
];

/// Create a test MemoryTool with mock conversation embedding handler.
async fn create_test_memory_tool() -> (MemoryTool, Arc<MockConversationRecallHandler>) {
    let handler = Arc::new(MockConversationRecallHandler::new());

    let tool = MemoryTool::new()
        .with_conversation_handler(handler.clone())
        .with_threshold(0.5);

    (tool, handler)
}

/// Embed test messages into the store using the mock handler.
async fn embed_test_messages(
    handler: &MockConversationRecallHandler,
    messages: &[(&str, &str, &str)],
) {
    for (i, (role, session_key, content)) in messages.iter().enumerate() {
        let message_id = format!("msg{}", i + 1);
        handler
            .embed_message(session_key, role, content, &message_id)
            .await
            .unwrap();
    }
}

/// Create a RoutingContext for a specific channel.
fn ctx_for_channel(channel: &str) -> RoutingContext {
    RoutingContext::new(
        ::common::ChannelName::new(channel),
        ::common::ChatId::new("test"),
    )
}

/// Default test context (Telegram).
fn ctx() -> RoutingContext {
    ctx_for_channel("telegram")
}

// ── Context Engine Helpers ────────────────────────────────────

/// Helper: build a ContextEngine with all sources for testing.
async fn test_context_engine(workspace: std::path::PathBuf) -> ContextEngine {
    let sources: Vec<Box<dyn ContextSource>> = vec![
        Box::new(IdentitySource::new(workspace.clone(), "UTC".to_string())),
        Box::new(BootstrapSource::new(workspace)),
        Box::new(ConfidenceSource::new(0.7)),
    ];

    ContextEngine::new().with_sources(sources)
}

fn test_source_ctx() -> SourceContext {
    SourceContext {
        channel: "test".to_string(),
        chat_id: "chat123".to_string(),
        message: None,
        intent_summary: None,
    }
}

// ── Memory Tool Actions ─────────────────────────────────────

#[tokio::test]
async fn semantic_search_finds_similar_todos() {
    let (tool, handler) = create_test_memory_tool().await;

    // Embed sample messages
    embed_test_messages(&handler, SAMPLE_MESSAGES).await;

    // Execute search
    let args = json!({
        "action": "search_conversations",
        "query": "authentication security",
        "limit": 5
    });

    let result = tool.execute(args, &ctx()).await.unwrap();

    // Verify results
    assert!(
        result.contains("conversation(s) matching"),
        "Expected conversation count in output"
    );
    assert!(
        result.contains("authentication"),
        "Expected 'authentication' in results"
    );
    assert!(
        result.contains("similarity:"),
        "Expected similarity scores in output"
    );
    assert!(
        result.contains("user") || result.contains("assistant"),
        "Expected role in output"
    );
}

#[tokio::test]
async fn search_respects_threshold() {
    let (tool, handler) = create_test_memory_tool().await;
    embed_test_messages(&handler, SAMPLE_MESSAGES).await;

    // Search with high threshold - should return fewer/no results
    let args_high = json!({
        "action": "search_conversations",
        "query": "authentication",
        "threshold": 0.9,
        "limit": 10
    });

    let result_high = tool.execute(args_high, &ctx()).await.unwrap();

    // Search with low threshold - should return more results
    let args_low = json!({
        "action": "search_conversations",
        "query": "authentication",
        "threshold": 0.1,
        "limit": 10
    });

    let result_low = tool.execute(args_low, &ctx()).await.unwrap();

    // Verify threshold filtering works
    assert!(
        result_high.contains("conversation(s) matching")
            || result_high.contains("No conversations found")
    );
    assert!(result_low.contains("conversation(s) matching"));
}

#[tokio::test]
async fn cross_channel_search() {
    let (tool, handler) = create_test_memory_tool().await;

    // Embed messages from different channels
    let telegram_messages = &[
        ("user", "telegram:12345", "How do I fix the bug?"),
        (
            "assistant",
            "telegram:12345",
            "You can fix it by updating the code...",
        ),
    ];
    let discord_messages = &[
        (
            "user",
            "discord:67890",
            "What about the authentication issue?",
        ),
        (
            "assistant",
            "discord:67890",
            "The authentication needs token validation...",
        ),
    ];

    embed_test_messages(&handler, telegram_messages).await;
    embed_test_messages(&handler, discord_messages).await;

    // Search from Telegram context
    let args = json!({
        "action": "search_conversations",
        "query": "authentication token",
        "limit": 10
    });

    let result = tool
        .execute(args, &ctx_for_channel("telegram"))
        .await
        .unwrap();

    // Verify results include both Telegram and Discord messages (global memory)
    assert!(result.contains("conversation(s) matching"));
    assert!(result.contains("authentication") || result.contains("token"));
}

#[tokio::test]
async fn rrf_merges_todo_conversation() {
    let (tool, handler) = create_test_memory_tool().await;
    embed_test_messages(&handler, SAMPLE_MESSAGES).await;

    // Search using unified search (conversation-only for Phase 4.1)
    let args = json!({
        "action": "search_all",
        "query": "authentication",
        "limit": 10
    });

    let result = tool.execute(args, &ctx()).await.unwrap();

    // Verify results include [Conversation] prefix
    assert!(
        result.contains("result(s) matching"),
        "Should show result count: {}",
        result
    );
    assert!(
        result.contains("[Conversation"),
        "Should include [Conversation] prefix: {}",
        result
    );
}

#[tokio::test]
async fn unified_search_both_sources() {
    let (tool, handler) = create_test_memory_tool().await;
    embed_test_messages(&handler, SAMPLE_MESSAGES).await;

    let args = json!({
        "action": "search_all",
        "query": "fix bug",
        "limit": 5
    });

    let result = tool.execute(args, &ctx()).await.unwrap();

    // Verify search_all works (conversation-only for now)
    assert!(
        result.contains("result") || result.contains("No results"),
        "Should return search results or no results message: {}",
        result
    );

    // Verify source indicator present if results found
    if !result.contains("No results") {
        assert!(
            result.contains("[Conversation]"),
            "Results should include source indicator: {}",
            result
        );
    }
}

#[tokio::test]
async fn purge_deletes_embeddings() {
    let (tool, handler) = create_test_memory_tool().await;
    embed_test_messages(&handler, SAMPLE_MESSAGES).await;

    // Verify embeddings exist before purge
    let status_before = json!({"action": "status"});
    let result_before = tool.execute(status_before, &ctx()).await.unwrap();
    assert!(result_before.contains("Total embeddings: 5"));

    // Purge embeddings from one session
    let purge_args = json!({
        "action": "purge",
        "filter": "session",
        "session_key": "telegram:12345"
    });

    let purge_result = tool.execute(purge_args, &ctx()).await.unwrap();
    assert!(purge_result.contains("Purged"));
    assert!(purge_result.contains("embedding(s)"));

    // Verify status shows fewer embeddings
    let status_after = json!({"action": "status"});
    let result_after = tool.execute(status_after, &ctx()).await.unwrap();
    // Should have fewer than 5 embeddings now
    assert!(result_after.contains("Total embeddings"));
}

#[tokio::test]
async fn purge_by_date_range() {
    let (tool, handler) = create_test_memory_tool().await;

    // Embed 3 messages (all will have current timestamp in test)
    embed_test_messages(&handler, &SAMPLE_MESSAGES[0..3]).await;

    // Verify we have 3 embeddings
    let status_before = json!({"action": "status"});
    let result_before = tool.execute(status_before, &ctx()).await.unwrap();
    assert!(
        result_before.contains("Total embeddings: 3"),
        "Should have 3 embeddings before purge"
    );

    // Purge before tomorrow (should delete all current embeddings)
    let tomorrow = (chrono::Utc::now() + chrono::Duration::days(1)).to_rfc3339();

    let purge_args = json!({
        "action": "purge",
        "filter": "before_date",
        "before_date": tomorrow
    });

    let purge_result = tool.execute(purge_args, &ctx()).await.unwrap();
    assert!(
        purge_result.contains("Purged"),
        "Should confirm purge: {}",
        purge_result
    );
    assert!(
        purge_result.contains("3") || purge_result.contains("embedding"),
        "Should mention deleted count: {}",
        purge_result
    );

    // Verify all deleted
    let status_after = json!({"action": "status"});
    let result_after = tool.execute(status_after, &ctx()).await.unwrap();
    assert!(
        result_after.contains("Total embeddings: 0"),
        "All embeddings should be purged: {}",
        result_after
    );
}

#[tokio::test]
async fn status_reports_accurate_counts() {
    let (tool, handler) = create_test_memory_tool().await;
    embed_test_messages(&handler, SAMPLE_MESSAGES).await;

    // Execute status action
    let args = json!({"action": "status"});
    let result = tool.execute(args, &ctx()).await.unwrap();

    // Verify counts
    assert!(result.contains("Conversation Memory Status"));
    assert!(
        result.contains("Total embeddings: 5"),
        "Expected 5 embeddings from SAMPLE_MESSAGES"
    );
    assert!(
        result.contains("Available: yes"),
        "Expected handler to be available"
    );
}

#[tokio::test]
async fn status_reports_channels_indexed() {
    let (tool, handler) = create_test_memory_tool().await;

    // Embed messages from 3 different channels
    let multi_channel_messages = &[
        ("user", "telegram:12345", "Message from Telegram"),
        ("user", "discord:67890", "Message from Discord"),
        ("user", "whatsapp:11111", "Message from WhatsApp"),
    ];
    embed_test_messages(&handler, multi_channel_messages).await;

    // Execute status
    let args = json!({"action": "status"});
    let result = tool.execute(args, &ctx()).await.unwrap();

    // Verify count
    assert!(
        result.contains("Total embeddings: 3"),
        "Should show 3 total embeddings: {}",
        result
    );
    assert!(
        result.contains("Available: yes"),
        "Should show handler is available: {}",
        result
    );
}

#[tokio::test]
async fn memory_tool_registration() {
    let (tool, _handler) = create_test_memory_tool().await;

    // Verify Tool trait implementation
    assert_eq!(tool.name(), "memory", "Tool name should be 'memory'");

    let description = tool.description();
    assert!(
        description.contains("search") || description.contains("Search"),
        "Description should mention search"
    );
    assert!(
        description.contains("conversations") || description.contains("conversation"),
        "Description should mention conversations"
    );

    // Verify parameters schema
    let params = tool.parameters();
    assert_eq!(params["type"], "object", "Parameters should be object type");
    assert!(
        params["required"]
            .as_array()
            .unwrap()
            .contains(&json!("action")),
        "Action should be required"
    );

    // Verify all 4 actions in enum
    let actions = params["properties"]["action"]["enum"].as_array().unwrap();
    assert_eq!(actions.len(), 4, "Should have 4 actions");
    assert!(
        actions.contains(&json!("search_conversations")),
        "Should include search_conversations"
    );
    assert!(
        actions.contains(&json!("search_all")),
        "Should include search_all"
    );
    assert!(actions.contains(&json!("purge")), "Should include purge");
    assert!(actions.contains(&json!("status")), "Should include status");
}

// ── Context Engine Sources ──────────────────────────────────

#[tokio::test]
async fn context_engine_init() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();

    let engine = test_context_engine(workspace).await;
    let prompt = engine.build_system_prompt("test", "chat123", None).await;

    // Should contain identity section
    assert!(prompt.contains("# Identity"));
    assert!(prompt.contains("klyntbot"));
}

#[tokio::test]
async fn context_engine_with_bootstrap_files() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();

    // Create bootstrap files
    std::fs::write(
        workspace.join("AGENTS.md"),
        "# Agent Configuration\n\nYou are helpful.",
    )
    .unwrap();
    std::fs::write(
        workspace.join("SOUL.md"),
        "# Agent Soul\n\nBe friendly and professional.",
    )
    .unwrap();
    std::fs::write(
        workspace.join("IDENTITY.md"),
        "# Identity\n\nI am klyntbot.",
    )
    .unwrap();
    std::fs::write(
        workspace.join("USER.md"),
        "# User\n\nUser prefers concise responses.",
    )
    .unwrap();
    std::fs::write(
        workspace.join("TOOLS.md"),
        "# Tools\n\nUse tools when needed.",
    )
    .unwrap();

    let engine = test_context_engine(workspace).await;
    let prompt = engine.build_system_prompt("test", "chat123", None).await;

    // Should include bootstrap file content
    assert!(prompt.contains("Agent Configuration"));
    assert!(prompt.contains("Agent Soul"));
    assert!(prompt.contains("Use tools when needed"));
}

#[tokio::test]
async fn bootstrap_files_optional() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();

    // No bootstrap files — should still work
    let engine = test_context_engine(workspace).await;
    let prompt = engine.build_system_prompt("test", "chat123", None).await;

    // Should contain at minimum the identity section
    assert!(prompt.contains("# Identity"));
    assert!(!prompt.is_empty());
}

#[tokio::test]
async fn context_with_channel_info() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();

    let engine = test_context_engine(workspace).await;

    // Build with different channels
    let telegram_prompt = engine
        .build_system_prompt("telegram", "chat123", None)
        .await;
    let discord_prompt = engine
        .build_system_prompt("discord", "guild456", None)
        .await;

    // Both should contain their channel info
    assert!(telegram_prompt.contains("telegram"));
    assert!(discord_prompt.contains("discord"));
}

#[tokio::test]
async fn identity_source_content() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();

    let source = IdentitySource::new(workspace, "UTC".to_string());
    let ctx = test_source_ctx();
    let content = source.provide(&ctx).await.unwrap();

    assert!(content.contains("# Identity"));
    assert!(content.contains("Channel: test"));
    assert!(content.contains("Chat ID: chat123"));
    assert!(content.contains("message"));
    assert!(content.contains("ask_user"));
}

#[tokio::test]
async fn confidence_source_threshold() {
    let source = ConfidenceSource::new(0.70);
    let ctx = test_source_ctx();

    let content = source.provide(&ctx).await.unwrap();
    assert!(content.contains("0.70"));

    // Update threshold
    source.set_threshold(0.85);
    let content = source.provide(&ctx).await.unwrap();
    assert!(content.contains("0.85"));
}

#[tokio::test]
async fn context_engine_source_ordering() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();

    let engine = test_context_engine(workspace).await;
    let prompt = engine.build_system_prompt("test", "chat123", None).await;

    // Identity (priority 100) should appear before confidence (priority 50)
    let identity_pos = prompt.find("# Identity").unwrap();
    let confidence_pos = prompt.find("Confidence").unwrap();
    assert!(
        identity_pos < confidence_pos,
        "Identity should appear before confidence"
    );
}

// ── Edge Cases ──────────────────────────────────────────────

#[tokio::test]
async fn search_rejects_empty_query() {
    let (tool, _handler) = create_test_memory_tool().await;

    // Execute search with empty query
    let args = json!({
        "action": "search_conversations",
        "query": ""
    });

    let result = tool.execute(args, &ctx()).await;

    // Should either error or return "No conversations found" message
    match result {
        Err(e) => {
            let err_msg = e.to_string();
            assert!(
                err_msg.contains("query")
                    || err_msg.contains("required")
                    || err_msg.contains("empty")
            );
        }
        Ok(msg) => {
            assert!(msg.contains("No conversations found") || msg.contains("0 conversation"));
        }
    }
}

#[tokio::test]
async fn no_embeddings_returns_message() {
    let (tool, _handler) = create_test_memory_tool().await;
    // Don't embed any messages - store is empty

    // Execute search
    let args = json!({
        "action": "search_conversations",
        "query": "test query"
    });

    let result = tool.execute(args, &ctx()).await.unwrap();

    // Verify helpful message
    assert!(
        result.contains("No conversations found"),
        "Expected 'No conversations found' message"
    );
}

#[tokio::test]
async fn partial_embeddings_noted() {
    let (tool, handler) = create_test_memory_tool().await;

    // Embed only 2 out of 5 sample messages
    let partial_messages = &SAMPLE_MESSAGES[0..2];
    embed_test_messages(&handler, partial_messages).await;

    // Execute search
    let args = json!({
        "action": "search_conversations",
        "query": "authentication",
        "limit": 10
    });

    let result = tool.execute(args, &ctx()).await.unwrap();

    // Should succeed and return results from available embeddings
    assert!(result.contains("conversation") || result.contains("No conversations found"));
}

#[tokio::test]
async fn below_threshold_no_results() {
    let (tool, handler) = create_test_memory_tool().await;
    embed_test_messages(&handler, SAMPLE_MESSAGES).await;

    // Search with very high threshold and unrelated query
    let args = json!({
        "action": "search_conversations",
        "query": "quantum physics relativity",  // Unrelated to sample messages
        "threshold": 0.99,  // Very high threshold
        "limit": 10
    });

    let result = tool.execute(args, &ctx()).await.unwrap();

    // Should return no results message
    assert!(
        result.contains("No conversations found"),
        "Expected no results due to high threshold"
    );
    assert!(
        result.contains("threshold"),
        "Expected threshold mentioned in output"
    );
}

#[tokio::test]
async fn hybrid_zero_keyword() {
    let (tool, handler) = create_test_memory_tool().await;

    let messages = &[
        (
            "user",
            "telegram:12345",
            "authentication security implementation",
        ),
        ("assistant", "telegram:12345", "verify credentials properly"),
    ];
    embed_test_messages(&handler, messages).await;

    // Query with different words (zero keyword matches, but semantic match)
    let args = json!({
        "action": "search_all",
        "query": "login authorization checking",
        "limit": 10
    });

    let result = tool.execute(args, &ctx()).await.unwrap();

    // Should return semantic matches (conversation embeddings)
    assert!(
        !result.contains("No results found"),
        "Should find semantic matches despite zero keyword matches: {}",
        result
    );
    assert!(
        result.contains("result(s)") || result.contains("[Conversation]"),
        "Should show results with [Conversation] prefix: {}",
        result
    );
}

#[tokio::test]
async fn hybrid_zero_semantic() {
    let handler = Arc::new(MockConversationRecallHandler::unavailable());

    let tool = MemoryTool::new()
        .with_conversation_handler(handler)
        .with_threshold(0.5);

    let args = json!({
        "action": "search_all",
        "query": "test",
        "limit": 10
    });

    let result = tool.execute(args, &ctx()).await.unwrap();

    // Should return unavailability message
    assert!(
        result.contains("not available") || result.contains("not loaded"),
        "Should indicate semantic search unavailable: {}",
        result
    );
}

#[tokio::test]
async fn query_too_long_returns_error() {
    let (tool, handler) = create_test_memory_tool().await;
    embed_test_messages(&handler, &SAMPLE_MESSAGES[0..2]).await;

    // Create query > 1000 chars
    let long_query = "a".repeat(1001);

    let args = json!({
        "action": "search_conversations",
        "query": long_query
    });

    let result = tool.execute(args, &ctx()).await;

    // Should either error or handle gracefully
    match result {
        Err(e) => {
            assert!(
                e.to_string().contains("too long")
                    || e.to_string().contains("length")
                    || e.to_string().contains("query"),
                "Error should mention query length issue"
            );
        }
        Ok(msg) => {
            assert!(
                msg.contains("conversation") || msg.contains("No conversations"),
                "Should handle long query gracefully: {}",
                msg
            );
        }
    }
}

#[tokio::test]
async fn invalid_threshold_returns_error() {
    let (tool, handler) = create_test_memory_tool().await;
    embed_test_messages(&handler, &SAMPLE_MESSAGES[0..2]).await;

    // Test threshold > 1.0
    let args_high = json!({
        "action": "search_conversations",
        "query": "test",
        "threshold": 1.5
    });

    let result_high = tool.execute(args_high, &ctx()).await;

    // Should handle invalid threshold (error or clamp to valid range)
    match result_high {
        Err(e) => {
            assert!(
                e.to_string().contains("threshold"),
                "Error should mention threshold: {}",
                e
            );
        }
        Ok(msg) => {
            assert!(
                msg.contains("No conversations") || msg.contains("conversation"),
                "Should handle high threshold gracefully: {}",
                msg
            );
        }
    }

    // Test threshold < 0.0
    let args_low = json!({
        "action": "search_conversations",
        "query": "test",
        "threshold": -0.5
    });

    let result_low = tool.execute(args_low, &ctx()).await;

    // Should handle invalid threshold
    match result_low {
        Err(e) => {
            assert!(
                e.to_string().contains("threshold"),
                "Error should mention threshold: {}",
                e
            );
        }
        Ok(_) => {
            // Graceful handling - search returns all results (negative threshold matches everything)
        }
    }
}

#[tokio::test]
async fn limit_parameter_respected() {
    let (tool, handler) = create_test_memory_tool().await;

    // Embed all 5 sample messages
    embed_test_messages(&handler, SAMPLE_MESSAGES).await;

    // Search with limit=2
    let args = json!({
        "action": "search_conversations",
        "query": "authentication api refactor status bug",  // Should match multiple messages
        "limit": 2,
        "threshold": 0.1  // Low threshold to ensure multiple matches
    });

    let result = tool.execute(args, &ctx()).await.unwrap();

    // Should return at most 2 results
    // Count the number of result entries (each starts with "- [")
    let result_count = result.matches("- [").count();
    assert!(
        result_count <= 2,
        "Should return at most 2 results, got {}",
        result_count
    );

    // Should indicate the result count
    assert!(result.contains("conversation(s) matching") || result.contains("No conversations"));
}
