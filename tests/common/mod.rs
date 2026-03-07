//! Shared test fixtures for integration tests.
//!
//! Provides reusable helpers for creating test configs, session managers,
//! message buses, mock providers, storage pools, and test data factories.
#![allow(dead_code)]

pub mod mocks;

use chrono::Utc;
use klyntbot::bus::{InboundMessage, MessageBus, OutboundMessage};
use klyntbot::config::Config;
use klyntbot::providers::types::*;
use klyntbot::session::SessionManager;
use std::sync::Arc;
use tempfile::TempDir;
use tools::todo_types::{Todo, TodoStatus};

// Re-exports for convenience (not all test binaries use every mock)
#[allow(unused_imports)]
pub use mocks::{
    ErrorProvider, MockConversationEmbeddingHandler, MockEmbeddingHandler, MockLearningHandler,
    MockProvider,
};

// ─── Config & workspace ──────────────────────────────────────

/// Create a Config with sensible test defaults.
///
/// The workspace path is set to a temporary directory that persists for the
/// lifetime of the returned `TempDir`. Callers should hold on to the `TempDir`
/// until the test is done to prevent premature cleanup.
pub fn test_config(temp_dir: &TempDir) -> Config {
    let mut config = Config::default();
    config.agents.defaults.workspace = temp_dir
        .path()
        .join("workspace")
        .to_str()
        .unwrap()
        .to_string();
    config.agents.defaults.model = "mock-model".to_string();
    config.agents.defaults.max_tokens = 1024;
    config.agents.defaults.temperature = 0.0;
    config.agents.defaults.max_tool_iterations = 5;
    config
}

/// Create a TempDir with an initialized workspace structure.
///
/// Returns the `TempDir` handle (drop it to clean up) and the workspace path.
pub fn test_workspace() -> (TempDir, std::path::PathBuf) {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let workspace = temp_dir.path().join("workspace");
    std::fs::create_dir_all(&workspace).expect("failed to create workspace dir");
    (temp_dir, workspace)
}

// ─── Session & bus ───────────────────────────────────────────

/// Create a SessionManager backed by a SQLite test pool.
pub async fn test_session_manager() -> (SessionManager, TempDir) {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let pool = test_pool().await;
    let repo = klyntbot::storage::SessionRepo::new(pool.inner().clone());
    let manager = SessionManager::from_repo(repo).await;
    (manager, temp_dir)
}

/// Create a MessageBus with a test-friendly buffer size.
pub fn test_message_bus() -> Arc<MessageBus> {
    Arc::new(MessageBus::new(100))
}

// ─── Mock provider helpers ───────────────────────────────────

/// Create a MockProvider that returns the given text.
pub fn test_provider(response: &str) -> MockProvider {
    MockProvider::new(response)
}

/// Create a MockProvider that returns a tool call.
pub fn test_provider_with_tool_call(name: &str, args: serde_json::Value) -> MockProvider {
    MockProvider::with_responses(vec![LlmResponse {
        content: None,
        tool_calls: vec![ToolCall {
            id: "test_call_1".to_string(),
            name: name.to_string(),
            arguments: args,
        }],
        finish_reason: "tool_calls".to_string(),
        usage: Usage::default(),
        reasoning_content: None,
    }])
}

// ─── LLM response factories ─────────────────────────────────

/// Build a tool-call LLM response.
pub fn tool_call_response(tool_name: &str, args: serde_json::Value) -> LlmResponse {
    LlmResponse {
        content: None,
        tool_calls: vec![ToolCall {
            id: "call_1".to_string(),
            name: tool_name.to_string(),
            arguments: args,
        }],
        finish_reason: "tool_calls".to_string(),
        usage: Usage::default(),
        reasoning_content: None,
    }
}

/// Build a text LLM response.
pub fn text_response(text: &str) -> LlmResponse {
    LlmResponse {
        content: Some(text.to_string()),
        tool_calls: vec![],
        finish_reason: "stop".to_string(),
        usage: Usage::default(),
        reasoning_content: None,
    }
}

// ─── Message factories ───────────────────────────────────────

/// Create a sample InboundMessage for testing.
pub fn sample_inbound(channel: &str, content: &str) -> InboundMessage {
    InboundMessage::new(channel, "test_user", "test_chat", content)
}

/// Create a sample OutboundMessage for testing.
pub fn sample_outbound(channel: &str, content: &str) -> OutboundMessage {
    OutboundMessage::new(channel, "test_chat", content)
}

// ─── Routing context ─────────────────────────────────────────

/// Create a RoutingContext for testing.
pub fn test_routing_ctx() -> tools::RoutingContext {
    tools::RoutingContext::new("cli".into(), "test".into())
}

/// Create a RoutingContext with specific channel/chat.
pub fn test_routing_ctx_for(channel: &str, chat_id: &str) -> tools::RoutingContext {
    tools::RoutingContext::new(channel.into(), chat_id.into())
}

// ─── Test data factories ─────────────────────────────────────

/// Create a Todo with sensible test defaults. All optional fields are None/empty.
pub fn create_test_todo(title: &str) -> Todo {
    Todo {
        id: Todo::generate_id(),
        title: title.to_string(),
        description: None,
        area_id: "test-area".to_string(),
        key_result_id: None,
        priority: None,
        due_date: None,
        tags: vec![],
        status: TodoStatus::Todo,
        focused_at: None,
        focus_deadline: None,
        focus_expired_count: 0,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        completed_at: None,
        parent_id: None,
        project_id: None,
        attachments: Vec::new(),
        time_entries: Vec::new(),
        total_tracked_secs: 0,
        estimated_minutes: None,
        calendar_event_uid: None,
        last_reminded_at: None,
        recurrence_rule: None,
        recurrence_parent_id: None,
        is_template: false,
        next_instance_date: None,
        blocked_by: Vec::new(),
        blocks: Vec::new(),
        status_label_id: None,
        position: 0,
        group_id: None,
    }
}

// ─── Storage pool & repo helpers ─────────────────────────────

/// Create a migrated in-memory SQLite StoragePool for testing.
///
/// Uses `StoragePool::connect_in_memory()` — no filesystem I/O, no leaked TempDirs.
pub async fn test_pool() -> klyntbot::storage::StoragePool {
    klyntbot::storage::StoragePool::connect_in_memory()
        .await
        .expect("in-memory SQLite test pool")
}

/// Create a `Repos` aggregate backed by an in-memory pool.
pub async fn test_repos() -> klyntbot::storage::Repos {
    klyntbot::storage::Repos::from_pool(&test_pool().await)
}

pub async fn test_action_repo() -> klyntbot::storage::ActionRepo {
    klyntbot::storage::ActionRepo::new(test_pool().await.inner().clone())
}

/// Create an AreaRepo backed by an in-memory pool.
pub async fn test_area_repo() -> klyntbot::storage::AreaRepo {
    klyntbot::storage::AreaRepo::new(test_pool().await.inner().clone())
}

/// Ensure the default test area ("test-area") exists in the given pool.
///
/// This must be called before inserting any ActionRows that reference "test-area".
pub async fn ensure_test_area(pool: &klyntbot::storage::StoragePool) {
    let area_repo = klyntbot::storage::AreaRepo::new(pool.inner().clone());
    let area = klyntbot::storage::AreaRow {
        id: "test-area".to_string(),
        name: "Test Area".to_string(),
        description: None,
        color: "#000000".to_string(),
        icon: None,
        position: 0,
        status: "active".to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    // Ignore error if already exists
    let _ = area_repo.create(&area).await;
}

pub async fn test_outcome_repo() -> klyntbot::storage::OutcomeRepo {
    klyntbot::storage::OutcomeRepo::new(test_pool().await.inner().clone())
}

pub async fn test_strategy_repo() -> klyntbot::storage::StrategyRepo {
    klyntbot::storage::StrategyRepo::new(test_pool().await.inner().clone())
}

pub async fn test_memory_note_repo() -> klyntbot::storage::MemoryNoteRepo {
    klyntbot::storage::MemoryNoteRepo::new(test_pool().await.inner().clone())
}
