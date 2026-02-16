//! Shared test fixtures for integration tests.
//!
//! Provides reusable helpers for creating test configs, session managers,
//! message buses, mock providers, and temporary workspaces.

use klyntbot::bus::{InboundMessage, MessageBus, OutboundMessage};
use klyntbot::config::Config;
use klyntbot::providers::types::*;
use klyntbot::session::SessionManager;
use std::sync::Arc;
use tempfile::TempDir;

// Re-export mock provider from existing test module
#[path = "../mock_provider.rs"]
pub mod mock_provider;

pub use mock_provider::MockProvider;

/// Create a Config with sensible test defaults.
///
/// The workspace path is set to a temporary directory that persists for the
/// lifetime of the returned `TempDir`.  Callers should hold on to the `TempDir`
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

/// Create a SessionManager backed by a temporary directory.
///
/// Returns the manager and a `TempDir` that must be kept alive.
pub async fn test_session_manager() -> (SessionManager, TempDir) {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let sessions_dir = temp_dir.path().join("sessions");
    std::fs::create_dir_all(&sessions_dir).expect("failed to create sessions dir");
    let manager = SessionManager::new(sessions_dir).await;
    (manager, temp_dir)
}

/// Create a MessageBus with a test-friendly buffer size.
pub fn test_message_bus() -> Arc<MessageBus> {
    Arc::new(MessageBus::new(100))
}

/// Create a MockProvider that returns the given text.
pub fn test_provider(response: &str) -> MockProvider {
    MockProvider::new(response)
}

/// Create a MockProvider that returns a tool call.
#[allow(dead_code)]
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

/// Create a sample InboundMessage for testing.
pub fn sample_inbound(channel: &str, content: &str) -> InboundMessage {
    InboundMessage::new(channel, "test_user", "test_chat", content)
}

/// Create a sample OutboundMessage for testing.
pub fn sample_outbound(channel: &str, content: &str) -> OutboundMessage {
    OutboundMessage::new(channel, "test_chat", content)
}

// ─── Sprint 5: Semantic Search helpers ────────────────────────

// Re-export mock embedding handler from existing test module
#[path = "../mock_embedding_handler.rs"]
pub mod mock_embedding_handler;

#[allow(unused_imports)]
pub use mock_embedding_handler::MockEmbeddingHandler;

use chrono::Utc;
use klyntbot::tools::todo_store::TodoStore;
use klyntbot::tools::todo_types::{Todo, TodoStatus};

/// Create a Todo with all fields explicitly initialized.
/// Follows the established convention from sprint2_deps_integration.rs.
#[allow(dead_code)]
pub fn create_test_todo(title: &str) -> Todo {
    Todo {
        id: Todo::generate_id(),
        title: title.to_string(),
        description: None,
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
    }
}

/// Create a Todo with tags for semantic search testing.
#[allow(dead_code)]
pub fn create_test_todo_with_tags(title: &str, tags: Vec<&str>) -> Todo {
    let mut todo = create_test_todo(title);
    todo.tags = tags.iter().map(|s| s.to_string()).collect();
    todo
}

/// Create a TodoStore backed by a temp directory.
#[allow(dead_code)]
pub async fn create_test_store() -> (TodoStore, TempDir) {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let file_path = temp_dir.path().join("todos.jsonl");
    let store = TodoStore::new(file_path);
    (store, temp_dir)
}

/// Create a TodoStore pre-populated with semantic test data.
///
/// Tasks cover the authentication/security/login domain for synonym testing,
/// plus unrelated tasks to verify precision.
#[allow(dead_code)]
pub async fn create_store_with_semantic_test_data() -> (TodoStore, TempDir) {
    let (mut store, temp_dir) = create_test_store().await;

    let tasks = vec![
        ("Fix authentication bug", vec!["backend", "security"]),
        ("Login system refactor", vec!["frontend", "auth"]),
        ("Update password hashing", vec!["security", "crypto"]),
        ("Add OAuth support", vec!["auth", "integration"]),
        ("Security audit", vec!["security", "review"]),
        ("Add dark mode toggle", vec!["frontend", "ui"]),
        ("Refactor database queries", vec!["backend", "performance"]),
        ("Write API documentation", vec!["docs", "api"]),
    ];

    for (title, tags) in tasks {
        let todo = create_test_todo_with_tags(title, tags);
        store.add(todo).await.unwrap();
    }

    (store, temp_dir)
}

/// Create a TodoStore with N todos for performance testing.
#[allow(dead_code)]
pub async fn create_store_with_n_todos(n: usize) -> (TodoStore, TempDir) {
    let (mut store, temp_dir) = create_test_store().await;
    for i in 0..n {
        let mut todo = create_test_todo(&format!("Task {} — various keywords for testing", i));
        todo.tags = vec![format!("tag-{}", i % 10), format!("category-{}", i % 5)];
        store.add(todo).await.unwrap();
    }
    (store, temp_dir)
}
