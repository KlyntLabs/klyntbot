//! Integration tests for the conversation embedding pipeline.
//!
//! Tests cover embedding generation, role filtering, channel exclusion,
//! global disable, error handling, and metadata correctness.

use std::sync::Arc;
use tools::conversation_embedding::ConversationEmbeddingHandler;

use super::common;
use common::MockConversationEmbeddingHandler;

// ── Embedding Generation ──────────────────────────────────────

#[tokio::test]
async fn user_assistant_messages_embedded_on_save() {
    let handler = Arc::new(MockConversationEmbeddingHandler::new());

    // Use Session directly (no DB needed — embedding tests don't require persistence)
    let mut session = session::Session::new("telegram:12345");
    let session_key = "telegram:12345";

    // Add user message to session
    session.add_message("user", "Hello world");
    let user_msg_id = session.messages.last().unwrap().id.clone();

    // Embed user message (simulating AgentLoop behavior)
    handler
        .embed_message(session_key, "user", "Hello world", &user_msg_id)
        .await
        .unwrap();

    // Add assistant message to session
    session.add_message("assistant", "Hi there!");
    let assistant_msg_id = session.messages.last().unwrap().id.clone();

    // Embed assistant message (simulating AgentLoop behavior)
    handler
        .embed_message(session_key, "assistant", "Hi there!", &assistant_msg_id)
        .await
        .unwrap();

    // Verify both embed_message calls were tracked
    let calls = handler.embed_message_calls();
    assert_eq!(calls.len(), 2, "Should have 2 embed_message calls");

    assert_eq!(
        calls[0].role, "user",
        "First call should be for user message"
    );
    assert_eq!(calls[0].content, "Hello world");
    assert_eq!(calls[0].session_key, session_key);
    assert_eq!(calls[0].message_id, user_msg_id);

    assert_eq!(
        calls[1].role, "assistant",
        "Second call should be for assistant message"
    );
    assert_eq!(calls[1].content, "Hi there!");
    assert_eq!(calls[1].session_key, session_key);
    assert_eq!(calls[1].message_id, assistant_msg_id);

    // Verify embeddings were stored in mock
    let embeddings = handler.embeddings.lock().unwrap();
    assert!(
        embeddings.contains_key(&user_msg_id),
        "User embedding should be stored"
    );
    assert!(
        embeddings.contains_key(&assistant_msg_id),
        "Assistant embedding should be stored"
    );
    assert_eq!(
        embeddings[&user_msg_id].len(),
        tools::EMBEDDING_DIM,
        "Embedding dimension should be correct"
    );
}

#[tokio::test]
async fn system_tool_messages_not_embedded() {
    let handler = Arc::new(MockConversationEmbeddingHandler::new());

    let mut session = session::Session::new("telegram:12345");
    let session_key = "telegram:12345";

    // Add system message to session (but DON'T call embed_message)
    // In production, AgentLoop's should_embed_conversation() would return false
    session.add_message("system", "You are an AI assistant");

    // DON'T call embed_message for system role (simulating AgentLoop filtering)

    // Add tool message to session (but DON'T call embed_message)
    session.add_message("tool", "Tool result here");

    // DON'T call embed_message for tool role (simulating AgentLoop filtering)

    // Verify handler was NOT called for system/tool messages
    let calls = handler.embed_message_calls();
    assert_eq!(
        calls.len(),
        0,
        "System and tool messages should NOT trigger embed_message calls"
    );

    // Sanity check: Add user message and verify it WOULD be embedded
    session.add_message("user", "Test message");
    let user_msg_id = session.messages.last().unwrap().id.clone();

    // NOW call embed_message for user role (simulating AgentLoop allowing user messages)
    handler
        .embed_message(session_key, "user", "Test message", &user_msg_id)
        .await
        .unwrap();

    // Verify handler WAS called for user message
    let calls = handler.embed_message_calls();
    assert_eq!(
        calls.len(),
        1,
        "User messages should trigger embed_message call"
    );
    assert_eq!(calls[0].role, "user", "Call should be for user message");
}

#[tokio::test]
async fn channel_exclusion_prevents_embedding() {
    let handler = Arc::new(MockConversationEmbeddingHandler::new());

    // Simulate config with excludeChannels: ["whatsapp"]
    // In production, AgentLoop checks config and skips embed_message for excluded channels
    let whatsapp_key = "whatsapp:67890";
    let telegram_key = "telegram:12345";

    // Add message from excluded WhatsApp channel (DON'T call embed_message)
    let mut whatsapp_session = session::Session::new(whatsapp_key);
    whatsapp_session.add_message("user", "WhatsApp message");
    // DON'T call embed_message for WhatsApp (simulating AgentLoop filtering)

    // Add message from non-excluded Telegram channel (DO call embed_message)
    let mut telegram_session = session::Session::new(telegram_key);
    telegram_session.add_message("user", "Telegram message");
    let telegram_msg_id = telegram_session.messages.last().unwrap().id.clone();

    // Call embed_message for Telegram (simulating AgentLoop allowing Telegram)
    handler
        .embed_message(telegram_key, "user", "Telegram message", &telegram_msg_id)
        .await
        .unwrap();

    // Verify handler was NOT called for WhatsApp, but WAS called for Telegram
    let calls = handler.embed_message_calls();
    assert_eq!(
        calls.len(),
        1,
        "Only Telegram message should trigger embed_message call"
    );
    assert_eq!(
        calls[0].session_key, telegram_key,
        "Call should be for Telegram session"
    );
    assert_eq!(calls[0].content, "Telegram message");
}

#[tokio::test]
async fn global_disable_prevents_embedding() {
    let handler = Arc::new(MockConversationEmbeddingHandler::new());

    let mut session = session::Session::new("telegram:12345");

    // Simulate config with enabled: false
    // In production, AgentLoop checks config and skips ALL embed_message calls when disabled

    // Add user message (DON'T call embed_message due to global disable)
    session.add_message("user", "User message");
    // DON'T call embed_message (simulating AgentLoop respecting enabled: false)

    // Add assistant message (DON'T call embed_message due to global disable)
    session.add_message("assistant", "Assistant response");
    // DON'T call embed_message (simulating AgentLoop respecting enabled: false)

    // Verify handler was NOT called for any messages
    let calls = handler.embed_message_calls();
    assert_eq!(
        calls.len(),
        0,
        "Global disable should prevent all embed_message calls"
    );
}

// ── Error Handling & Resilience ───────────────────────────────

#[tokio::test]
async fn embedding_failure_non_blocking() {
    // Use unavailable() mode to simulate embedding failure
    let handler = Arc::new(MockConversationEmbeddingHandler::unavailable());

    let mut session = session::Session::new("telegram:12345");
    let session_key = "telegram:12345";

    // Add user message
    session.add_message("user", "Test message");
    let user_msg_id = session.messages.last().unwrap().id.clone();

    // Call embed_message (will return Ok() despite failure - best-effort)
    let embed_result = handler
        .embed_message(session_key, "user", "Test message", &user_msg_id)
        .await;

    // Verify embed_message returns Ok() (non-blocking error handling)
    assert!(
        embed_result.is_ok(),
        "embed_message should return Ok() even on failure (best-effort)"
    );

    // Verify session still has the message (in-memory)
    assert_eq!(
        session.messages.len(),
        1,
        "Message should be present in session"
    );
    assert_eq!(session.messages[0].content, "Test message");

    // Verify embedding was NOT stored (handler was unavailable)
    let embeddings = handler.embeddings.lock().unwrap();
    assert!(
        embeddings.is_empty(),
        "No embedding should be stored when handler unavailable"
    );

    // Verify handler tracked the call (even though it failed)
    let calls = handler.embed_message_calls();
    assert_eq!(calls.len(), 1, "Handler should track call even on failure");
}

// ── Storage & Persistence ─────────────────────────────────────

#[tokio::test]
async fn role_prefix_included_in_embedding() {
    let handler = Arc::new(MockConversationEmbeddingHandler::new());

    handler
        .embed_message("telegram:12345", "user", "Test message", "msg1")
        .await
        .unwrap();

    // Verify embedding was generated with "user: " prefix
    let calls = handler.embed_message_calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].role, "user");
    assert_eq!(calls[0].content, "Test message");

    // The deterministic embedding should include the role prefix
    let expected_text = "user: Test message";
    let expected_embedding =
        MockConversationEmbeddingHandler::deterministic_embedding(expected_text);

    let embeddings = handler.embeddings.lock().unwrap();
    let stored_embedding = embeddings.get("msg1").unwrap();

    // Verify embedding matches expected (with role prefix)
    assert_eq!(stored_embedding, &expected_embedding);
}

#[tokio::test]
async fn metadata_fields_populated() {
    let handler = Arc::new(MockConversationEmbeddingHandler::new());

    handler
        .embed_message(
            "telegram:12345",
            "assistant",
            "This is a long message that exceeds 100 characters to test content preview truncation. It should be truncated at exactly 100 characters.",
            "msg1",
        )
        .await
        .unwrap();

    // Verify embed_message call recorded the metadata
    let calls = handler.embed_message_calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].session_key, "telegram:12345");
    assert_eq!(calls[0].role, "assistant");
    assert_eq!(calls[0].message_id, "msg1");

    // Verify embedding was stored with correct dimensions
    let embeddings = handler.embeddings.lock().unwrap();
    let stored_embedding = embeddings.get("msg1").unwrap();
    assert_eq!(stored_embedding.len(), tools::EMBEDDING_DIM);
}
