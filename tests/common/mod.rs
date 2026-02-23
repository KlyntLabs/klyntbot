//! Shared test fixtures for integration tests.
//!
//! Provides reusable helpers for creating test configs, session managers,
//! message buses, mock providers, and temporary workspaces.
#![allow(dead_code)]

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

/// Create a SessionManager backed by a SQLite test pool.
///
/// Returns the manager and a `TempDir` that must be kept alive (for other temp artifacts).
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

// ─── Storage repo helpers (SQLite temp-file pool) ─────────────

/// Create a SQLite StoragePool backed by a temporary directory.
///
/// The temp directory is leaked intentionally — the OS cleans it up on
/// process exit, which is acceptable for ephemeral test processes.
pub async fn test_pool() -> klyntbot::storage::StoragePool {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let pool = klyntbot::storage::StoragePool::connect(dir.path())
        .await
        .expect("SQLite test pool");
    std::mem::forget(dir);
    pool
}

pub async fn test_todo_repo() -> klyntbot::storage::TodoRepo {
    klyntbot::storage::TodoRepo::new(test_pool().await.inner().clone())
}

pub async fn test_outcome_repo() -> klyntbot::storage::OutcomeRepo {
    klyntbot::storage::OutcomeRepo::new(test_pool().await.inner().clone())
}

pub async fn test_learning_state_repo() -> klyntbot::storage::LearningStateRepo {
    klyntbot::storage::LearningStateRepo::new(test_pool().await.inner().clone())
}

pub async fn test_memory_note_repo() -> klyntbot::storage::MemoryNoteRepo {
    klyntbot::storage::MemoryNoteRepo::new(test_pool().await.inner().clone())
}

pub async fn test_calendar_sync_repo() -> klyntbot::storage::CalendarSyncRepo {
    klyntbot::storage::CalendarSyncRepo::new(test_pool().await.inner().clone())
}

pub async fn test_event_cache_repo() -> klyntbot::storage::CalendarEventCacheRepo {
    klyntbot::storage::CalendarEventCacheRepo::new(test_pool().await.inner().clone())
}
