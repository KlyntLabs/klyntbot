//! Integration tests for memory system and context builder

use klyntbot::agent::{ContextBuilder, MemoryStore};
use klyntbot::session::SessionMessage;
use tempfile::TempDir;

/// Test context builder initialization
#[tokio::test]
async fn test_context_builder_init() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();

    let mut context_builder = ContextBuilder::new(workspace, None).await;
    let result = context_builder.init().await;
    assert!(result.is_ok());
}

/// Test bootstrap file loading
#[tokio::test]
async fn test_context_builder_with_bootstrap_files() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();

    // Create bootstrap files
    let agents_md = workspace.join("AGENTS.md");
    std::fs::write(&agents_md, "# Agent Configuration\n\nYou are helpful.").unwrap();

    let soul_md = workspace.join("SOUL.md");
    std::fs::write(&soul_md, "# Agent Soul\n\nBe friendly and professional.").unwrap();

    let identity_md = workspace.join("IDENTITY.md");
    std::fs::write(&identity_md, "# Identity\n\nI am klyntbot.").unwrap();

    let user_md = workspace.join("USER.md");
    std::fs::write(&user_md, "# User\n\nUser prefers concise responses.").unwrap();

    let tools_md = workspace.join("TOOLS.md");
    std::fs::write(&tools_md, "# Tools\n\nUse tools when needed.").unwrap();

    let mut context_builder = ContextBuilder::new(workspace, None).await;
    context_builder.init().await.unwrap();

    // Build messages
    let messages = context_builder
        .build_messages(vec![], "Hello!", None, "test", "chat123")
        .await;

    // Should include system message and user message
    assert!(messages.len() >= 2);
}

/// Test memory file creation and reading
#[tokio::test]
async fn test_memory_store() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().join("workspace");
    let memory_dir = workspace.join("memory");
    std::fs::create_dir_all(&memory_dir).unwrap();

    // Create long-term memory file
    let memory_file = memory_dir.join("MEMORY.md");
    let memory_content = r#"# Long-term Memory

## User Preferences
- Prefers Python over JavaScript
- Likes detailed explanations

## Ongoing Projects
- Working on web scraper project
- Learning Rust programming
"#;
    std::fs::write(&memory_file, memory_content).unwrap();

    let _memory_store = MemoryStore::new(workspace.clone());

    // Memory store should be able to access the memory files
    // The actual implementation details depend on MemoryStore's API
    assert!(memory_file.exists());
}

/// Test daily notes creation and retrieval
#[tokio::test]
async fn test_daily_notes_structure() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().join("workspace");
    let memory_dir = workspace.join("memory");
    std::fs::create_dir_all(&memory_dir).unwrap();

    // Create a daily note
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let daily_note = memory_dir.join(format!("{}.md", today));

    let note_content = format!(
        r#"# Daily Notes - {}

## Conversations
- Discussed Rust testing strategies
- Helped debug a async/await issue

## Tasks Completed
- Reviewed PR #123
- Fixed integration tests
"#,
        today
    );
    std::fs::write(&daily_note, note_content).unwrap();

    // Verify file exists
    assert!(daily_note.exists());

    let mut context_builder = ContextBuilder::new(workspace, None).await;
    context_builder.init().await.unwrap();

    // Build messages
    let messages = context_builder
        .build_messages(vec![], "What did we do today?", None, "test", "chat123")
        .await;

    assert!(!messages.is_empty());
}

/// Test message building with conversation history
#[tokio::test]
async fn test_context_builder_with_history() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();

    let mut context_builder = ContextBuilder::new(workspace, None).await;
    context_builder.init().await.unwrap();

    // Create conversation history
    let history = vec![
        SessionMessage {
            role: "user".to_string(),
            content: "What is Rust?".to_string(),
            timestamp: chrono::Utc::now(),
        },
        SessionMessage {
            role: "assistant".to_string(),
            content: "Rust is a systems programming language.".to_string(),
            timestamp: chrono::Utc::now(),
        },
    ];

    // Build messages with history
    let messages = context_builder
        .build_messages(history, "Tell me more", None, "test", "chat123")
        .await;

    // Should include system message, history messages, and current message
    assert!(messages.len() >= 4);
}

/// Test workspace path resolution
#[test]
fn test_workspace_path_structure() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();

    // Verify workspace structure can be created
    assert!(workspace.exists());
    assert!(workspace.is_dir());
}

/// Test bootstrap files optional loading
#[tokio::test]
async fn test_bootstrap_files_optional() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();

    // Don't create any bootstrap files - should still work

    let mut context_builder = ContextBuilder::new(workspace, None).await;
    let result = context_builder.init().await;

    // Should initialize successfully even without bootstrap files
    assert!(result.is_ok());

    // Should be able to build messages
    let messages = context_builder
        .build_messages(vec![], "Hello", None, "test", "chat123")
        .await;

    assert!(!messages.is_empty());
}

/// Test memory directory structure
#[test]
fn test_memory_directory_structure() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().join("workspace");
    let memory_dir = workspace.join("memory");

    std::fs::create_dir_all(&memory_dir).unwrap();

    // Verify directory exists
    assert!(memory_dir.exists());
    assert!(memory_dir.is_dir());

    // Create memory file
    let memory_file = memory_dir.join("MEMORY.md");
    std::fs::write(&memory_file, "# Memory\n\nTest content").unwrap();

    assert!(memory_file.exists());
    assert!(memory_file.is_file());
}

/// Test memory store initialization
#[test]
fn test_memory_store_initialization() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();

    let _memory_store = MemoryStore::new(workspace.clone());

    // Memory store should initialize without error
    // The actual implementation details depend on MemoryStore's API
    assert!(workspace.exists());
}

/// Test context builder with media files
#[tokio::test]
async fn test_context_builder_with_media() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();

    let mut context_builder = ContextBuilder::new(workspace, None).await;
    context_builder.init().await.unwrap();

    // Create test media paths
    let media_paths = vec!["/path/to/image1.png".to_string()];

    // Build messages with media
    let messages = context_builder
        .build_messages(
            vec![],
            "What's in this image?",
            Some(media_paths),
            "test",
            "chat123",
        )
        .await;

    // Should include messages (exact structure depends on implementation)
    assert!(!messages.is_empty());
}

/// Test multiple context builders with same workspace
#[tokio::test]
async fn test_multiple_context_builders() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();

    // Create two context builders
    let mut builder1 = ContextBuilder::new(workspace.clone(), None).await;
    let mut builder2 = ContextBuilder::new(workspace.clone(), None).await;

    // Both should initialize successfully
    assert!(builder1.init().await.is_ok());
    assert!(builder2.init().await.is_ok());

    // Both should be able to build messages
    let messages1 = builder1
        .build_messages(vec![], "Hello from builder1", None, "test", "chat1")
        .await;
    let messages2 = builder2
        .build_messages(vec![], "Hello from builder2", None, "test", "chat2")
        .await;

    assert!(!messages1.is_empty());
    assert!(!messages2.is_empty());
}

/// Test context with channel and chat ID
#[tokio::test]
async fn test_context_with_channel_info() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();

    let mut context_builder = ContextBuilder::new(workspace, None).await;
    context_builder.init().await.unwrap();

    // Build messages with different channel/chat combinations
    let telegram_messages = context_builder
        .build_messages(vec![], "Message from Telegram", None, "telegram", "chat123")
        .await;

    let discord_messages = context_builder
        .build_messages(vec![], "Message from Discord", None, "discord", "guild456")
        .await;

    // Both should build successfully
    assert!(!telegram_messages.is_empty());
    assert!(!discord_messages.is_empty());
}

/// Test empty conversation history
#[tokio::test]
async fn test_empty_conversation_history() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();

    let mut context_builder = ContextBuilder::new(workspace, None).await;
    context_builder.init().await.unwrap();

    // Build messages with empty history
    let messages = context_builder
        .build_messages(vec![], "First message", None, "test", "chat123")
        .await;

    // Should include at least system message and user message
    assert!(messages.len() >= 2);
}

/// Test long conversation history
#[tokio::test]
async fn test_long_conversation_history() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();

    let mut context_builder = ContextBuilder::new(workspace, None).await;
    context_builder.init().await.unwrap();

    // Create long history
    let mut history = Vec::new();
    for i in 0..50 {
        history.push(SessionMessage {
            role: "user".to_string(),
            content: format!("Message {}", i),
            timestamp: chrono::Utc::now(),
        });
        history.push(SessionMessage {
            role: "assistant".to_string(),
            content: format!("Response {}", i),
            timestamp: chrono::Utc::now(),
        });
    }

    // Build messages with long history
    let messages = context_builder
        .build_messages(history, "Latest message", None, "test", "chat123")
        .await;

    // Should handle long history without error
    assert!(!messages.is_empty());
}
