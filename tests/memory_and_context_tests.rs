//! Integration tests for memory system and context engine (sources)

use klyntbot::agent::context_sources::{
    BootstrapSource, ConfidenceSource, IdentitySource, MemorySource, SkillContentSource,
    SkillSummarySource,
};
use klyntbot::agent::{MemoryStore, SkillManager};
use klyntbot::context_engine::{ContextEngine, ContextSource, SourceContext};
use std::sync::Arc;
use tempfile::TempDir;

/// Helper: create a MemoryNoteRepo backed by an in-memory SQLite pool.
///
/// Uses in-memory SQLite (no filesystem I/O) since these tests never query the DB.
async fn test_memory_note_repo() -> klyntbot::storage::MemoryNoteRepo {
    let pool = sqlx::SqlitePool::connect(":memory:")
        .await
        .expect("in-memory SQLite");
    klyntbot::storage::MemoryNoteRepo::new(pool)
}

/// Helper: build a ContextEngine with all sources for testing.
async fn test_context_engine(workspace: std::path::PathBuf) -> ContextEngine {
    let mut skill_manager = SkillManager::new();
    let _ = skill_manager.load(workspace.clone()).await;
    let skill_manager = Arc::new(skill_manager);

    let memory_store = MemoryStore::new(test_memory_note_repo().await);

    let sources: Vec<Box<dyn ContextSource>> = vec![
        Box::new(IdentitySource::new(workspace.clone(), "UTC".to_string())),
        Box::new(BootstrapSource::new(workspace)),
        Box::new(MemorySource::new(memory_store)),
        Box::new(ConfidenceSource::new(0.7)),
        Box::new(SkillSummarySource::new(Arc::clone(&skill_manager))),
        Box::new(SkillContentSource::new(skill_manager)),
    ];

    ContextEngine::new().with_sources(sources)
}

fn test_source_ctx() -> SourceContext {
    SourceContext {
        channel: "test".to_string(),
        chat_id: "chat123".to_string(),
        message: None,
    }
}

/// Test context engine initialization with sources
#[tokio::test]
async fn test_context_engine_init() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();

    let engine = test_context_engine(workspace).await;
    let prompt = engine.build_system_prompt("test", "chat123", None).await;

    // Should contain identity section
    assert!(prompt.contains("# Identity"));
    assert!(prompt.contains("klyntbot"));
}

/// Test bootstrap file loading
#[tokio::test]
async fn test_context_engine_with_bootstrap_files() {
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

/// Test memory file creation and reading
#[tokio::test]
async fn test_memory_store() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().join("workspace");
    let memory_dir = workspace.join("memory");
    std::fs::create_dir_all(&memory_dir).unwrap();

    let memory_file = memory_dir.join("MEMORY.md");
    let memory_content = r#"# Long-term Memory

## User Preferences
- Prefers Python over JavaScript
- Likes detailed explanations
"#;
    std::fs::write(&memory_file, memory_content).unwrap();

    let _memory_store = MemoryStore::new(test_memory_note_repo().await);

    assert!(memory_file.exists());
}

/// Test workspace path structure
#[test]
fn test_workspace_path_structure() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();

    assert!(workspace.exists());
    assert!(workspace.is_dir());
}

/// Test bootstrap files are optional
#[tokio::test]
async fn test_bootstrap_files_optional() {
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

/// Test memory directory structure
#[test]
fn test_memory_directory_structure() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().join("workspace");
    let memory_dir = workspace.join("memory");
    std::fs::create_dir_all(&memory_dir).unwrap();

    assert!(memory_dir.exists());
    assert!(memory_dir.is_dir());

    let memory_file = memory_dir.join("MEMORY.md");
    std::fs::write(&memory_file, "# Memory\n\nTest content").unwrap();
    assert!(memory_file.exists());
    assert!(memory_file.is_file());
}

/// Test memory store initialization
#[tokio::test]
async fn test_memory_store_initialization() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();

    let _memory_store = MemoryStore::new(test_memory_note_repo().await);
    assert!(workspace.exists());
}

/// Test context with channel and chat ID
#[tokio::test]
async fn test_context_with_channel_info() {
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

/// Test identity source contains expected sections
#[tokio::test]
async fn test_identity_source_content() {
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

/// Test confidence source with threshold update
#[tokio::test]
async fn test_confidence_source_threshold() {
    let source = ConfidenceSource::new(0.70);
    let ctx = test_source_ctx();

    let content = source.provide(&ctx).await.unwrap();
    assert!(content.contains("0.70"));

    // Update threshold
    source.set_threshold(0.85);
    let content = source.provide(&ctx).await.unwrap();
    assert!(content.contains("0.85"));
}

/// Test that sources are ordered by priority
#[tokio::test]
async fn test_context_engine_source_ordering() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();

    let engine = test_context_engine(workspace).await;
    let prompt = engine.build_system_prompt("test", "chat123", None).await;

    // Identity (priority 100) should appear before skills summary (priority 40)
    let identity_pos = prompt.find("# Identity").unwrap();
    let skills_pos = prompt.find("# Available Skills").unwrap();
    assert!(
        identity_pos < skills_pos,
        "Identity should appear before skills"
    );
}
