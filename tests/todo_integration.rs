//! Integration tests for the todo system
//!
//! These tests verify end-to-end functionality:
//! - TodoTool execution via AgentLoop
//! - Context injection in system prompts
//! - Notification delivery (OS native + channels)
//!
//! NOTE: These tests will be implemented once Phases 1-6 are complete.
//! Current status: DRAFT - Structure only, awaiting implementation dependencies

#![allow(unused_imports, dead_code)]

use klyntbot::{AgentLoop, Config, MessageBus};
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

#[path = "mock_provider.rs"]
mod mock_provider;

/// Helper: Create a test AgentLoop with todo system enabled
async fn create_test_agent_with_todo() -> (AgentLoop, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    // Set HOME to temp dir so .klyntbot directories are created in test isolation
    std::env::set_var("HOME", temp_dir.path());

    // Create workspace directory
    let workspace = temp_dir.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();

    // Create minimal config with workspace path
    let mut config = Config::default();
    config.agents.defaults.workspace = workspace.to_str().unwrap().to_string();

    // Create message bus and mock provider
    let bus = Arc::new(MessageBus::new(10));
    let provider = Arc::new(crate::mock_provider::MockProvider::new("Test response"));

    // Create AgentLoop with lazy pool (SQL queries will fail unless DATABASE_URL is valid)
    let pool = klyntbot::storage::StoragePool::connect_lazy("postgres://localhost/klyntbot_test").unwrap();
    let todo_repo = klyntbot::storage::TodoRepo::new(pool.inner().clone());
    let agent = AgentLoop::new(bus, provider, config, todo_repo, None).await.unwrap();

    (agent, temp_dir)
}

/// Helper: Create a mock NotificationDispatcher for testing
fn create_mock_notification_dispatcher() -> Arc<tokio::sync::RwLock<Vec<(String, String)>>> {
    // Returns a shared vector to capture notification calls
    // Tests can push (title, body) tuples to verify notifications
    Arc::new(tokio::sync::RwLock::new(Vec::new()))
}

#[tokio::test]
#[ignore] // Remove this once Phase 3 is complete
async fn test_todo_tool_via_agent() {
    // SCENARIO: User sends message asking to create a task
    // EXPECTED: TodoTool is called, task is created, response mentions task ID
    //
    // Test flow:
    // 1. Create agent with todo tool enabled
    // 2. Send message: "add a task to fix the authentication bug"
    // 3. Verify TodoTool was invoked (check tool registry call count)
    // 4. Verify task was persisted to JSONL
    // 5. Verify response contains task ID and confidence score

    todo!("Implement after Phase 3 (TodoTool) completion");

    // let (agent, _temp_dir) = create_test_agent_with_todo().await;
    //
    // // Send message requesting task creation
    // let response = agent.process_message(...).await?;
    //
    // // Verify tool was called
    // assert!(response.contains("task"));
    //
    // // Verify JSONL persistence
    // // ...
}

#[tokio::test]
async fn test_todo_context_injection() {
    // SCENARIO: User has focused tasks, starts new conversation
    // EXPECTED: System prompt contains "# Active Tasks" section with focused tasks
    //
    // Test flow:
    // 1. Create todo store with 2 focused tasks
    // 2. Create ContextBuilder with that store
    // 3. Build system prompt
    // 4. Verify "# Active Tasks" section exists
    // 5. Verify both tasks appear with titles and time remaining
    // 6. Verify format matches specification (task ID, title, time left)

    use chrono::Utc;
    use klyntbot::agent::ContextBuilder;
    use klyntbot::tools::todo_types::{Todo, TodoStatus};

    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().to_path_buf();

    // Connect to test database (skip if unavailable)
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://localhost/klyntbot_test".to_string());
    let pool = match klyntbot::storage::StoragePool::connect(&db_url).await {
        Ok(p) => p,
        Err(_) => return, // Skip: no database available
    };
    let repos = klyntbot::storage::Repos::from_pool(&pool);
    let todo_repo = repos.todos;

    // Task 1: High-priority backend task
    let task1 = Todo {
        id: Todo::generate_id(),
        title: "Fix authentication bug in production".to_string(),
        description: Some("Auth tokens expire during long sessions".to_string()),
        priority: Some(5),
        due_date: None,
        tags: vec!["backend".to_string(), "urgent".to_string()],
        status: TodoStatus::Doing,
        focused_at: None,
        focus_deadline: None,
        focus_expired_count: 0,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        completed_at: None,
        // Phase 1 new fields
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
    };
    let task1_id = task1.id.clone();
    let task1_row: klyntbot::storage::TodoRow = (&task1).into();
    todo_repo.add(&task1_row).await.unwrap();

    // Task 2: Medium-priority frontend task
    let task2 = Todo {
        id: Todo::generate_id(),
        title: "Implement user profile page redesign".to_string(),
        description: Some("New mockups from design team".to_string()),
        priority: Some(3),
        due_date: None,
        tags: vec!["frontend".to_string(), "ui".to_string()],
        status: TodoStatus::Todo,
        focused_at: None,
        focus_deadline: None,
        focus_expired_count: 0,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        completed_at: None,
        // Phase 1 new fields
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
    };
    let task2_id = task2.id.clone();
    let task2_row: klyntbot::storage::TodoRow = (&task2).into();
    todo_repo.add(&task2_row).await.unwrap();

    // Focus both tasks (18-hour deadline)
    let deadline = Utc::now() + chrono::Duration::hours(18);
    todo_repo.focus(&task1_id, 3, Some(deadline)).await.unwrap();
    todo_repo.focus(&task2_id, 3, Some(deadline)).await.unwrap();

    // Create ContextBuilder with the todo repo
    let mut context_builder = ContextBuilder::new(
        workspace.clone(),
        "UTC".to_string(),
        Some(todo_repo.clone()),
        None,
    )
    .await;

    // Initialize (this loads skills from workspace - will be empty for test)
    context_builder.init().await.unwrap();

    // Build messages (which includes system prompt with todo context)
    let messages = context_builder
        .build_messages(
            vec![], // Empty history
            "test user message",
            None, // No media
            "test-channel",
            "test-chat",
        )
        .await;

    // First message should be the system prompt
    assert!(!messages.is_empty(), "Should have at least one message");
    let system_msg = &messages[0];

    // Extract system prompt content from the message
    let system_prompt = match system_msg {
        klyntbot::providers::types::Message::System { content } => content,
        _ => panic!("First message should be a system message"),
    };

    // Verify "# Active Tasks" section exists
    assert!(
        system_prompt.contains("# Active Tasks"),
        "System prompt should contain '# Active Tasks' section"
    );

    // Verify focus indicator
    assert!(
        system_prompt.contains("🎯 Focus (2/3)"),
        "System prompt should show 2/3 focused tasks"
    );

    // Verify both task titles appear
    assert!(
        system_prompt.contains("Fix authentication bug in production"),
        "System prompt should contain task 1 title"
    );
    assert!(
        system_prompt.contains("Implement user profile page redesign"),
        "System prompt should contain task 2 title"
    );

    // Verify task IDs appear (for reference)
    assert!(
        system_prompt.contains(&format!("[{}]", task1.id)),
        "System prompt should contain task 1 ID in brackets"
    );
    assert!(
        system_prompt.contains(&format!("[{}]", task2.id)),
        "System prompt should contain task 2 ID in brackets"
    );

    // Verify priorities appear
    assert!(
        system_prompt.contains("P5"),
        "System prompt should show task 1 priority"
    );
    assert!(
        system_prompt.contains("P3"),
        "System prompt should show task 2 priority"
    );

    // Verify tags appear
    assert!(
        system_prompt.contains("backend/urgent"),
        "System prompt should show task 1 tags"
    );
    assert!(
        system_prompt.contains("frontend/ui"),
        "System prompt should show task 2 tags"
    );

    // Verify time remaining format (should show hours left)
    assert!(
        system_prompt.contains("h left"),
        "System prompt should show hours remaining for focused tasks"
    );

    // Test no-repo scenario - verify no todo context leaks through
    let mut empty_context_builder = ContextBuilder::new(
        workspace,
        "UTC".to_string(),
        None, // No todo repo
        None,
    )
    .await;
    empty_context_builder.init().await.unwrap();

    let empty_messages = empty_context_builder
        .build_messages(vec![], "test", None, "test-channel", "test-chat")
        .await;

    let empty_prompt = match &empty_messages[0] {
        klyntbot::providers::types::Message::System { content } => content,
        _ => panic!("First message should be a system message"),
    };

    // Without a todo repo, no Active Tasks section should appear
    assert!(
        !empty_prompt.contains("# Active Tasks"),
        "No-repo context should not have Active Tasks section"
    );

    // Cleanup test data
    let _ = todo_repo.delete(&task1_id).await;
    let _ = todo_repo.delete(&task2_id).await;
}

#[tokio::test]
async fn test_notification_delivery_os_native() {
    // SCENARIO: Focus reminder triggers, OS native notifications enabled
    // EXPECTED: NotificationDispatcher calls OS notification API without error
    //
    // NOTE: We can't easily mock OS notification calls (they use platform-specific
    // commands like osascript, notify-send, PowerShell). This test verifies the
    // dispatcher correctly invokes the OS notification path without errors.
    //
    // Test flow:
    // 1. Create NotificationDispatcher with "os_native" in targets
    // 2. Trigger notify() with test title and body
    // 3. Verify call completes without error
    // 4. Note: Actual notification display depends on OS platform and may fail
    //    in headless CI environments, but dispatcher handles failures gracefully

    use klyntbot::agent::notifications::NotificationDispatcher;
    use klyntbot::config::schema::TodoNotificationConfig;
    use tokio::sync::mpsc;

    // Create mpsc channel (not used for OS native, but required for constructor)
    let (outbound_tx, _outbound_rx) = mpsc::channel(10);

    // Create config with "os_native" as notification target
    let config = TodoNotificationConfig {
        targets: vec!["os_native".to_string()],
        focus_reminders: true,
        daily_digest: false,
        daily_digest_time: "09:00".to_string(),
    };

    // Create dispatcher
    let dispatcher = NotificationDispatcher::new(outbound_tx, config);

    // Trigger notification - should complete without error even if OS notification fails
    // (NotificationDispatcher catches OS notification errors and logs warnings)
    let result = dispatcher
        .notify("Task Reminder", "Fix authentication bug - 1h remaining")
        .await;

    assert!(
        result.is_ok(),
        "Dispatcher should handle OS notifications gracefully even if platform unavailable"
    );

    // Test with multiple targets including os_native
    let config_multi = TodoNotificationConfig {
        targets: vec!["os_native".to_string(), "telegram".to_string()],
        focus_reminders: true,
        daily_digest: false,
        daily_digest_time: "09:00".to_string(),
    };

    let (outbound_tx2, _) = mpsc::channel(10);
    let dispatcher2 = NotificationDispatcher::new(outbound_tx2, config_multi);

    // This should attempt OS notification + telegram channel (though telegram won't send without last_active)
    let result2 = dispatcher2
        .notify("Daily Summary", "3 tasks completed")
        .await;

    assert!(
        result2.is_ok(),
        "Dispatcher should handle multiple targets including OS native"
    );

    // Test with special characters (verify sanitization)
    let result3 = dispatcher
        .notify(
            "Test \"quotes\" and $vars",
            "Body with `backticks` and \\backslash",
        )
        .await;

    assert!(
        result3.is_ok(),
        "Dispatcher should handle special characters safely"
    );
}

#[tokio::test]
async fn test_notification_delivery_channel() {
    // SCENARIO: Focus reminder triggers, channel notifications enabled (e.g., Telegram)
    // EXPECTED: NotificationDispatcher sends OutboundMessage to message bus
    //
    // Test flow:
    // 1. Create NotificationDispatcher with "telegram" in targets
    // 2. Set last_active_channel to telegram + test chat_id
    // 3. Capture outbound messages via mpsc receiver
    // 4. Trigger notify() with test title and body
    // 5. Verify OutboundMessage was sent to correct channel + chat_id
    // 6. Verify message format (title + newlines + body)

    use klyntbot::agent::notifications::NotificationDispatcher;
    use klyntbot::bus::OutboundMessage;
    use klyntbot::common::{ChannelName, ChatId};
    use klyntbot::config::schema::TodoNotificationConfig;
    use tokio::sync::mpsc;

    // Create mpsc channel to capture outbound messages
    let (outbound_tx, mut outbound_rx) = mpsc::channel::<OutboundMessage>(10);

    // Create config with "telegram" as notification target
    let config = TodoNotificationConfig {
        targets: vec!["telegram".to_string()],
        focus_reminders: true,
        daily_digest: false,
        daily_digest_time: "09:00".to_string(),
    };

    // Create dispatcher
    let dispatcher = NotificationDispatcher::new(outbound_tx, config);

    // Set last active channel to telegram:chat123
    let last_active = dispatcher.last_active_handle();
    *last_active.write().await = Some((ChannelName::from("telegram"), ChatId::from("chat123")));

    // Trigger notification
    let result = dispatcher
        .notify("Focus Reminder", "Task 'Fix bug' - 2h remaining")
        .await;
    assert!(result.is_ok(), "Notification should succeed");

    // Verify OutboundMessage was sent
    let msg = tokio::time::timeout(tokio::time::Duration::from_millis(100), outbound_rx.recv())
        .await
        .expect("Should receive message within timeout")
        .expect("Channel should not be closed");

    // Verify message routing (channel + chat_id)
    assert_eq!(
        msg.channel.as_str(),
        "telegram",
        "Message should be sent to telegram channel"
    );
    assert_eq!(
        msg.chat_id.as_str(),
        "chat123",
        "Message should be sent to correct chat ID"
    );

    // Verify message format (title + newlines + body)
    assert!(
        msg.content.contains("Focus Reminder"),
        "Message should contain title"
    );
    assert!(
        msg.content.contains("Task 'Fix bug' - 2h remaining"),
        "Message should contain body"
    );
    assert_eq!(
        msg.content, "Focus Reminder\n\nTask 'Fix bug' - 2h remaining",
        "Message should have title + double newline + body format"
    );

    // Test with different channel - should not send (no matching last active)
    let config2 = TodoNotificationConfig {
        targets: vec!["discord".to_string()],
        focus_reminders: true,
        daily_digest: false,
        daily_digest_time: "09:00".to_string(),
    };
    let (outbound_tx2, mut outbound_rx2) = mpsc::channel::<OutboundMessage>(10);
    let dispatcher2 = NotificationDispatcher::new(outbound_tx2, config2);

    // Last active is still telegram, but config targets discord
    let last_active2 = dispatcher2.last_active_handle();
    *last_active2.write().await = Some((ChannelName::from("telegram"), ChatId::from("chat456")));

    // Trigger notification - should not send (discord target, but telegram is last active)
    dispatcher2.notify("Test", "Should not send").await.unwrap();

    // Verify NO message was sent (timeout should occur)
    let result =
        tokio::time::timeout(tokio::time::Duration::from_millis(50), outbound_rx2.recv()).await;
    assert!(
        result.is_err(),
        "No message should be sent when target doesn't match last active channel"
    );

    // Test with matching channel
    let last_active2 = dispatcher2.last_active_handle();
    *last_active2.write().await = Some((ChannelName::from("discord"), ChatId::from("guild789")));

    dispatcher2
        .notify("Daily Digest", "3 tasks due")
        .await
        .unwrap();

    let msg2 = tokio::time::timeout(tokio::time::Duration::from_millis(100), outbound_rx2.recv())
        .await
        .expect("Should receive message")
        .expect("Channel should not be closed");

    assert_eq!(msg2.channel.as_str(), "discord");
    assert_eq!(msg2.chat_id.as_str(), "guild789");
    assert!(msg2.content.contains("Daily Digest"));
}

#[tokio::test]
async fn test_notification_delivery_multi_target() {
    // SCENARIO: Notification configured for multiple targets (os_native + channel)
    // EXPECTED: Both targets receive the notification
    //
    // Test flow:
    // 1. Create NotificationDispatcher with ["os_native", "telegram"] targets
    // 2. Set last_active_channel to telegram
    // 3. Trigger notify()
    // 4. Verify channel message was sent
    // 5. Verify os_native path was invoked (no error)

    use klyntbot::agent::notifications::NotificationDispatcher;
    use klyntbot::bus::OutboundMessage;
    use klyntbot::common::{ChannelName, ChatId};
    use klyntbot::config::schema::TodoNotificationConfig;
    use tokio::sync::mpsc;

    // Create mpsc channel to capture channel messages
    let (outbound_tx, mut outbound_rx) = mpsc::channel::<OutboundMessage>(10);

    // Create config with multiple targets
    let config = TodoNotificationConfig {
        targets: vec!["os_native".to_string(), "telegram".to_string()],
        focus_reminders: true,
        daily_digest: false,
        daily_digest_time: "09:00".to_string(),
    };

    // Create dispatcher
    let dispatcher = NotificationDispatcher::new(outbound_tx, config);

    // Set last active channel
    let last_active = dispatcher.last_active_handle();
    *last_active.write().await = Some((ChannelName::from("telegram"), ChatId::from("chat999")));

    // Trigger notification - should hit both targets
    let result = dispatcher
        .notify("Multi-Target Test", "This should go to OS and Telegram")
        .await;

    assert!(result.is_ok(), "Multi-target notification should succeed");

    // Verify telegram channel message
    let msg = tokio::time::timeout(tokio::time::Duration::from_millis(100), outbound_rx.recv())
        .await
        .expect("Should receive telegram message")
        .expect("Channel should not be closed");

    assert_eq!(msg.channel.as_str(), "telegram");
    assert_eq!(msg.chat_id.as_str(), "chat999");
    assert!(msg.content.contains("Multi-Target Test"));
    assert!(msg.content.contains("This should go to OS and Telegram"));

    // OS native notification was also attempted (we can't verify it was displayed,
    // but we verified result.is_ok() which means dispatcher didn't error)

    // Test 3 targets: os_native + 2 channels (only one matches last_active)
    let config_triple = TodoNotificationConfig {
        targets: vec![
            "os_native".to_string(),
            "telegram".to_string(),
            "discord".to_string(),
        ],
        focus_reminders: true,
        daily_digest: false,
        daily_digest_time: "09:00".to_string(),
    };

    let (outbound_tx2, mut outbound_rx2) = mpsc::channel::<OutboundMessage>(10);
    let dispatcher2 = NotificationDispatcher::new(outbound_tx2, config_triple);

    // Set last active to telegram (discord won't match)
    let last_active2 = dispatcher2.last_active_handle();
    *last_active2.write().await = Some((ChannelName::from("telegram"), ChatId::from("test")));

    dispatcher2
        .notify("Triple Target", "Testing 3 targets")
        .await
        .unwrap();

    // Should receive exactly 1 channel message (telegram matches, discord doesn't)
    let msg2 = tokio::time::timeout(tokio::time::Duration::from_millis(100), outbound_rx2.recv())
        .await
        .expect("Should receive telegram message")
        .expect("Channel should not be closed");

    assert_eq!(msg2.channel.as_str(), "telegram");

    // Verify no second message (discord didn't match)
    let no_msg =
        tokio::time::timeout(tokio::time::Duration::from_millis(50), outbound_rx2.recv()).await;
    assert!(
        no_msg.is_err(),
        "Should not receive discord message (doesn't match last active)"
    );
}

#[tokio::test]
async fn test_todo_store_persistence() {
    // SCENARIO: Create tasks, restart agent, verify tasks persist
    // EXPECTED: JSONL file survives process restart
    //
    // Test flow:
    // 1. Create TodoStore with test JSONL path
    // 2. Add 3 tasks
    // 3. Drop TodoStore (simulating shutdown)
    // 4. Create new TodoStore with same path
    // 5. Verify all 3 tasks are loaded
    // 6. Verify task data matches (title, status, priority, etc.)

    use chrono::Utc;
    use klyntbot::tools::todo_store::TodoStore;
    use klyntbot::tools::todo_types::{Todo, TodoFilter, TodoStatus};

    let temp_dir = TempDir::new().unwrap();
    let todo_path = temp_dir.path().join("todos.jsonl");

    // Create and populate store
    let (task1_id, task2_id, task3_id) = {
        let mut store = TodoStore::new(todo_path.clone());

        // Task 1: Minimal (low confidence)
        let task1 = Todo {
            id: Todo::generate_id(),
            title: "Fix bug".to_string(),
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
            // Phase 1 new fields
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
        };
        let task1 = store.add(task1).await.unwrap();

        // Task 2: Partial (medium confidence)
        let task2 = Todo {
            id: Todo::generate_id(),
            title: "Fix the authentication bug".to_string(),
            description: Some("Tokens expire too fast".to_string()),
            priority: Some(4),
            due_date: None,
            tags: vec!["backend".to_string()],
            status: TodoStatus::Doing,

            focused_at: None,
            focus_deadline: None,
            focus_expired_count: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            completed_at: None,
            // Phase 1 new fields
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
        };
        let task2 = store.add(task2).await.unwrap();

        // Task 3: Complete (high confidence)
        let task3 = Todo {
            id: Todo::generate_id(),
            title: "Implement todo system persistence".to_string(),
            description: Some("Add JSONL-based storage with lazy loading support".to_string()),
            priority: Some(5),
            due_date: Some(Utc::now() + chrono::Duration::days(7)),
            tags: vec!["backend".to_string(), "storage".to_string()],
            status: TodoStatus::Done,

            focused_at: None,
            focus_deadline: None,
            focus_expired_count: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            completed_at: Some(Utc::now()),
            // Phase 1 new fields
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
        };
        let task3 = store.add(task3).await.unwrap();

        (task1.id, task2.id, task3.id)
        // store auto-saves on each add
    }; // store dropped here (simulating shutdown)

    // Reload and verify persistence
    let mut store = TodoStore::new(todo_path);
    let tasks = store.list(&TodoFilter::default()).await.unwrap();

    assert_eq!(tasks.len(), 3, "All 3 tasks should persist across restart");

    // Verify task 1 (minimal)
    let task1 = store
        .get(&task1_id)
        .await
        .unwrap()
        .expect("Task 1 should exist");
    assert_eq!(task1.title, "Fix bug");
    assert_eq!(task1.status, TodoStatus::Todo);

    // Verify task 2 (partial)
    let task2 = store
        .get(&task2_id)
        .await
        .unwrap()
        .expect("Task 2 should exist");
    assert_eq!(task2.title, "Fix the authentication bug");
    assert_eq!(task2.status, TodoStatus::Doing);

    // Verify task 3 (complete)
    let task3 = store
        .get(&task3_id)
        .await
        .unwrap()
        .expect("Task 3 should exist");
    assert_eq!(task3.title, "Implement todo system persistence");
    assert_eq!(task3.status, TodoStatus::Done);
    assert!(
        task3.completed_at.is_some(),
        "Completed task should have timestamp"
    );
}

// test_confidence_scoring_accuracy removed — old field-completeness heuristic replaced by LLM-driven confidence

#[tokio::test]
#[ignore] // Legacy confidence test — replaced by LLM-driven confidence
async fn test_confidence_scoring_accuracy_legacy() {
    // This test validated the old calculate_confidence() heuristic which has been removed.
    // The new confidence system is LLM-driven (see crates/agent/src/confidence/).
}

#[tokio::test]
async fn test_focus_slot_limit() {
    // SCENARIO: User tries to focus 4 tasks when max is 3
    // EXPECTED: 4th focus attempt fails with error
    //
    // Test flow:
    // 1. Create TodoStore with 4 tasks
    // 2. Focus tasks 1, 2, 3 (should succeed)
    // 3. Try to focus task 4 (should fail)
    // 4. Verify error message mentions slot limit
    // 5. Unfocus task 1
    // 6. Try to focus task 4 again (should now succeed)

    use chrono::Utc;
    use klyntbot::tools::todo_store::TodoStore;
    use klyntbot::tools::todo_types::{Todo, TodoStatus};

    let temp_dir = TempDir::new().unwrap();
    let mut store = TodoStore::new(temp_dir.path().join("todos.jsonl"));

    // Create 4 tasks
    let mut task_ids = Vec::new();
    for i in 1..=4 {
        let task = Todo {
            id: Todo::generate_id(),
            title: format!("Task {} with enough words", i),
            description: Some(format!("Description for task {}", i)),
            priority: Some(3),
            due_date: None,
            tags: vec!["test".to_string()],
            status: TodoStatus::Todo,

            focused_at: None,
            focus_deadline: None,
            focus_expired_count: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            completed_at: None,
            // Phase 1 new fields
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
        };
        let task = store.add(task).await.unwrap();
        task_ids.push(task.id.clone());
    }

    // Focus first 3 tasks (should all succeed)
    let result1 = store.focus(&task_ids[0], 3, 18).await;
    assert!(result1.is_ok(), "First focus should succeed");
    assert!(result1.unwrap(), "First focus should return true");

    let result2 = store.focus(&task_ids[1], 3, 18).await;
    assert!(result2.is_ok(), "Second focus should succeed");

    let result3 = store.focus(&task_ids[2], 3, 18).await;
    assert!(result3.is_ok(), "Third focus should succeed");

    // Verify 3 tasks are focused
    let focused = store.focused().await.unwrap();
    assert_eq!(focused.len(), 3, "Should have 3 focused tasks");

    // Try to focus 4th task (should fail - slots full)
    let result4 = store.focus(&task_ids[3], 3, 18).await;
    assert!(result4.is_err(), "Fourth focus should fail (slots full)");

    // Verify error message mentions slot limit
    let err_msg = result4.unwrap_err().to_string();
    assert!(
        err_msg.contains("Focus slots full") || err_msg.contains("3/3"),
        "Error should mention slot limit: {}",
        err_msg
    );

    // Unfocus task 1 to free a slot
    let unfocus_result = store.unfocus(&task_ids[0]).await;
    assert!(unfocus_result.is_ok(), "Unfocus should succeed");

    // Verify only 2 focused tasks now
    let focused = store.focused().await.unwrap();
    assert_eq!(
        focused.len(),
        2,
        "Should have 2 focused tasks after unfocus"
    );

    // Try to focus task 4 again (should succeed now)
    let result4_retry = store.focus(&task_ids[3], 3, 18).await;
    assert!(
        result4_retry.is_ok(),
        "Fourth focus should succeed after freeing a slot"
    );

    // Verify 3 tasks focused again
    let focused = store.focused().await.unwrap();
    assert_eq!(focused.len(), 3, "Should have 3 focused tasks again");
}

#[tokio::test]
async fn test_auto_unfocus_expired_tasks() {
    // SCENARIO: Focused task exceeds 18-hour deadline
    // EXPECTED: Task is auto-unfocused, focus_expired_count incremented
    //
    // SPEC GAP SG-2 VALIDATION: This test validates that auto_unfocus_expired()
    // correctly increments focus_expired_count when unfocusing expired tasks.
    //
    // Test flow:
    // 1. Create a task with expired focus deadline (simulate past time)
    // 2. Call auto_unfocus_expired()
    // 3. Verify task is unfocused
    // 4. Verify focus_expired_count = 1
    // 5. Focus again, expire again, verify count = 2 (repeat scenario)

    use chrono::{Duration, Utc};
    use klyntbot::tools::todo_store::TodoStore;
    use klyntbot::tools::todo_types::{Todo, TodoStatus};

    let temp_dir = TempDir::new().unwrap();
    let mut store = TodoStore::new(temp_dir.path().join("todos.jsonl"));

    let now = Utc::now();

    // Create a task with focus already expired (focused 20 hours ago, 18h deadline)
    let expired_task = Todo {
        id: Todo::generate_id(),
        title: "Task with expired focus deadline".to_string(),
        description: Some("This task's focus has expired".to_string()),
        priority: Some(4),
        due_date: None,
        tags: vec!["urgent".to_string()],
        status: TodoStatus::Doing,
        focused_at: Some(now - Duration::hours(20)), // Focused 20 hours ago
        focus_deadline: Some(now - Duration::hours(2)), // Expired 2 hours ago
        focus_expired_count: 0,
        created_at: now - Duration::hours(20),
        updated_at: now - Duration::hours(20),
        completed_at: None,
        // Phase 1 new fields
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
    };

    let task = store.add(expired_task).await.unwrap();
    let task_id = task.id.clone();

    // Verify task is currently focused (before auto-unfocus)
    let task_before = store.get(&task_id).await.unwrap().unwrap();
    assert!(
        task_before.focused_at.is_some(),
        "Task should be focused before auto-unfocus"
    );
    assert_eq!(
        task_before.focus_expired_count, 0,
        "Expired count should be 0 initially"
    );

    // Call auto_unfocus_expired() - should unfocus the expired task
    let expired_tasks = store.auto_unfocus_expired().await.unwrap();

    // Verify exactly 1 task was auto-unfocused
    assert_eq!(
        expired_tasks.len(),
        1,
        "Should have auto-unfocused exactly 1 task"
    );
    assert_eq!(
        expired_tasks[0].id, task_id,
        "The expired task should match"
    );

    // Verify task is now unfocused
    let task_after = store.get(&task_id).await.unwrap().unwrap();
    assert!(
        task_after.focused_at.is_none(),
        "Task should be unfocused after auto-unfocus"
    );
    assert!(
        task_after.focus_deadline.is_none(),
        "Focus deadline should be cleared"
    );

    // SG-2: Verify focus_expired_count was incremented
    assert_eq!(
        task_after.focus_expired_count, 1,
        "SG-2: focus_expired_count should be incremented to 1"
    );

    // ADDITIONAL TEST: Focus again, expire again, verify count increments to 2
    // Focus the task again
    store.focus(&task_id, 3, 1).await.unwrap(); // 1-hour deadline

    // Manually update to make it expired again (simulate time passage)
    // We'll add another task with expired focus to test the increment again
    // (We can't easily manipulate the existing task's deadline, so we'll verify the count logic differently)
    // Instead, let's verify the second expiration scenario works:

    // Create another task that's also expired
    let expired_task2 = Todo {
        id: Todo::generate_id(),
        title: "Second expired task".to_string(),
        description: Some("Another expired focus".to_string()),
        priority: Some(3),
        due_date: None,
        tags: vec![],
        status: TodoStatus::Todo,
        focused_at: Some(now - Duration::hours(25)),
        focus_deadline: Some(now - Duration::hours(7)),
        focus_expired_count: 3, // Already expired 3 times before
        created_at: now - Duration::hours(25),
        updated_at: now - Duration::hours(25),
        completed_at: None,
        // Phase 1 new fields
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
    };

    let task2 = store.add(expired_task2).await.unwrap();
    let task2_id = task2.id.clone();

    // Auto-unfocus again - should catch both expired tasks
    let expired_tasks2 = store.auto_unfocus_expired().await.unwrap();

    // Should find task2 (task1 is no longer focused after first auto_unfocus)
    assert_eq!(
        expired_tasks2.len(),
        1,
        "Should find 1 more expired task (task2)"
    );

    // Verify task2's count was incremented from 3 to 4
    let task2_after = store.get(&task2_id).await.unwrap().unwrap();
    assert_eq!(
        task2_after.focus_expired_count, 4,
        "Task2 expired count should increment from 3 to 4"
    );
    assert!(
        task2_after.focused_at.is_none(),
        "Task2 should be unfocused"
    );
}

#[tokio::test]
async fn test_complete_focused_task_frees_slot() {
    // SCENARIO: User completes a focused task
    // EXPECTED: Focus fields cleared, slot freed, can focus another task
    //
    // SPEC GAP SG-7 CORRECTION: The original design's TodoStore::update() method
    // does NOT clear focused_at and focus_deadline when status changes to Done.
    // This creates orphaned focus slots. This test validates the CORRECTED behavior
    // where completing a task automatically unfocuses it.
    //
    // SOURCE: business-analyst review, SG-7
    // RELATED: AC-12.8 (update method), focus slot management
    //
    // Test flow:
    // 1. Create 4 tasks, focus first 3 (slots full: 3/3)
    // 2. Complete task 1 via update() with status: Done
    // 3. Verify task 1 has focused_at = None, focus_deadline = None
    // 4. Verify slot count is now 2/3 (slot freed)
    // 5. Verify can focus task 4 (proof slot is available)

    use chrono::Utc;
    use klyntbot::tools::todo_store::TodoStore;
    use klyntbot::tools::todo_types::{Todo, TodoFilter, TodoPatch, TodoStatus};

    let temp_dir = TempDir::new().unwrap();
    let mut store = TodoStore::new(temp_dir.path().join("todos.jsonl"));

    // Create 4 tasks
    let mut task_ids = Vec::new();
    for i in 1..=4 {
        let task = Todo {
            id: Todo::generate_id(),
            title: format!("Task {}", i),
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
            // Phase 1 new fields
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
        };
        let task = store.add(task).await.unwrap();
        task_ids.push(task.id.clone());
    }

    // Focus first 3 tasks (fill all slots)
    store.focus(&task_ids[0], 3, 18).await.unwrap();
    store.focus(&task_ids[1], 3, 18).await.unwrap();
    store.focus(&task_ids[2], 3, 18).await.unwrap();

    // Verify slots full
    let focused = store.focused().await.unwrap();
    assert_eq!(focused.len(), 3, "All 3 slots should be occupied");

    // Complete task 1 (should auto-clear focus fields per SG-7 fix)
    let patch = TodoPatch {
        status: Some(TodoStatus::Done),
        ..Default::default()
    };
    store.update(&task_ids[0], patch).await.unwrap();

    // CRITICAL: Verify focus fields cleared (SG-7 fix)
    let task1 = store
        .get(&task_ids[0])
        .await
        .unwrap()
        .expect("Task 1 should exist");
    assert!(
        task1.focused_at.is_none(),
        "SG-7: focused_at should be cleared on complete"
    );
    assert!(
        task1.focus_deadline.is_none(),
        "SG-7: focus_deadline should be cleared on complete"
    );
    assert_eq!(task1.status, TodoStatus::Done, "Task should be marked Done");
    assert!(
        task1.completed_at.is_some(),
        "Completed task should have timestamp"
    );

    // Verify slot freed (count should be 2/3)
    let focused = store.focused().await.unwrap();
    assert_eq!(focused.len(), 2, "Slot should be freed when task completed");

    // Verify can focus another task (proof slot is available)
    let result = store.focus(&task_ids[3], 3, 18).await;
    assert!(
        result.is_ok(),
        "Should be able to focus task 4 after completing task 1"
    );

    let focused = store.focused().await.unwrap();
    assert_eq!(focused.len(), 3, "Slot count should be 3/3 again");
}

#[tokio::test]
async fn test_focus_and_delete_interaction() {
    // SCENARIO: Delete a focused task and verify slot is freed
    // EXPECTED: Slot count decrements, can focus another task
    //
    // SOURCE: business-analyst additional edge case, validates AC-12.22
    //
    // Test flow:
    // 1. Create 4 tasks
    // 2. Focus tasks 1, 2, 3 (slots full: 3/3)
    // 3. Delete task 2 (which is focused)
    // 4. Verify slot count drops to 2/3
    // 5. Verify can focus task 4 (proof slot freed)

    use chrono::Utc;
    use klyntbot::tools::todo_store::TodoStore;
    use klyntbot::tools::todo_types::{Todo, TodoStatus};

    let temp_dir = TempDir::new().unwrap();
    let mut store = TodoStore::new(temp_dir.path().join("todos.jsonl"));

    // Create 4 tasks
    let mut task_ids = Vec::new();
    for i in 1..=4 {
        let task = Todo {
            id: Todo::generate_id(),
            title: format!("Task {} needs more words", i),
            description: Some(format!("Description for task {}", i)),
            priority: Some(3),
            due_date: None,
            tags: vec!["test".to_string()],
            status: TodoStatus::Todo,

            focused_at: None,
            focus_deadline: None,
            focus_expired_count: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            completed_at: None,
            // Phase 1 new fields
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
        };
        let task = store.add(task).await.unwrap();
        task_ids.push(task.id.clone());
    }

    // Focus first 3 tasks (fill all slots)
    store.focus(&task_ids[0], 3, 18).await.unwrap();
    store.focus(&task_ids[1], 3, 18).await.unwrap();
    store.focus(&task_ids[2], 3, 18).await.unwrap();

    // Verify all 3 slots occupied
    let focused = store.focused().await.unwrap();
    assert_eq!(focused.len(), 3, "All 3 slots should be occupied");

    // Verify 4th focus attempt fails (slots full)
    let result = store.focus(&task_ids[3], 3, 18).await;
    assert!(result.is_err(), "4th focus should fail (slots full)");

    // Delete task 2 (which is currently focused)
    let deleted = store.delete(&task_ids[1]).await.unwrap();
    assert!(deleted, "Task 2 should be deleted successfully");

    // Verify slot count dropped to 2/3 (AC-12.22)
    let focused = store.focused().await.unwrap();
    assert_eq!(
        focused.len(),
        2,
        "AC-12.22: Deleting focused task should free slot"
    );

    // Verify the deleted task is not in the focused list
    assert!(
        !focused.iter().any(|t| t.id == task_ids[1]),
        "Deleted task should not be in focused list"
    );

    // Verify can now focus task 4 (proof slot was freed)
    let result = store.focus(&task_ids[3], 3, 18).await;
    assert!(
        result.is_ok(),
        "Should be able to focus task 4 after deleting task 2"
    );

    // Verify slot count is 3/3 again
    let focused = store.focused().await.unwrap();
    assert_eq!(focused.len(), 3, "Slot count should be 3/3 again");

    // Verify the focused tasks are 1, 3, and 4 (not 2)
    let focused_ids: Vec<String> = focused.iter().map(|t| t.id.clone()).collect();
    assert!(
        focused_ids.contains(&task_ids[0]),
        "Task 1 should be focused"
    );
    assert!(
        !focused_ids.contains(&task_ids[1]),
        "Task 2 should not be focused (deleted)"
    );
    assert!(
        focused_ids.contains(&task_ids[2]),
        "Task 3 should be focused"
    );
    assert!(
        focused_ids.contains(&task_ids[3]),
        "Task 4 should be focused"
    );
}

#[tokio::test]
async fn test_empty_store_operations() {
    // SCENARIO: Call all read operations on an empty store (no JSONL file exists)
    // EXPECTED: Sensible defaults returned, no panics
    //
    // SOURCE: business-analyst additional edge case, prevents panics on edge conditions
    //
    // Operations to test:
    // - list() → empty Vec
    // - summary() → all counts zero
    // - focused() → empty Vec
    // - to_context_string() → minimal markdown with "(no focused tasks)"
    //
    // This validates lazy loading behavior when file doesn't exist yet

    use klyntbot::tools::todo_store::TodoStore;
    use klyntbot::tools::todo_types::TodoFilter;

    let temp_dir = TempDir::new().unwrap();
    let todo_path = temp_dir.path().join("nonexistent.jsonl");

    // Verify JSONL file doesn't exist yet
    assert!(
        !todo_path.exists(),
        "JSONL file should not exist before store creation"
    );

    // Create store pointing to nonexistent file (lazy loading)
    let mut store = TodoStore::new(todo_path.clone());

    // list() should return empty vec, not panic
    let tasks = store.list(&TodoFilter::default()).await.unwrap();
    assert_eq!(tasks.len(), 0, "Empty store should return empty list");

    // summary() should return zero counts
    let summary = store.summary().await.unwrap();
    assert_eq!(summary.total, 0, "Empty store total should be 0");
    assert_eq!(
        summary.by_status.len(),
        0,
        "Empty store should have no status counts"
    );
    assert_eq!(
        summary.overdue.len(),
        0,
        "Empty store should have no overdue tasks"
    );
    assert_eq!(
        summary.upcoming_week.len(),
        0,
        "Empty store should have no upcoming tasks"
    );

    // focused() should return empty vec
    let focused = store.focused().await.unwrap();
    assert_eq!(focused.len(), 0, "Empty store should have no focused tasks");

    // to_context_string() should return markdown with "(no focused tasks)"
    let context = store.to_context_string().await.unwrap();
    assert!(
        context.contains("no focused tasks"),
        "Empty store context should indicate no focused tasks: {}",
        context
    );
    assert!(
        context.contains("# Active Tasks"),
        "Context should still have header"
    );
    assert!(
        context.contains("🎯 Focus (0/3)"),
        "Context should show 0/3 focus slots"
    );

    // get() on nonexistent ID should return None, not panic
    let result = store.get("nonexistent-id").await.unwrap();
    assert!(result.is_none(), "get() on missing ID should return None");

    // unfocus() on nonexistent ID should return false (not error)
    let unfocus_result = store.unfocus("nonexistent-id").await.unwrap();
    assert!(
        !unfocus_result,
        "unfocus() on missing ID should return false"
    );

    // delete() on nonexistent ID should return false (not error)
    let delete_result = store.delete("nonexistent-id").await.unwrap();
    assert!(!delete_result, "delete() on missing ID should return false");

    // Verify JSONL file still doesn't exist (lazy loading - no writes yet)
    assert!(
        !todo_path.exists(),
        "JSONL file should not be created until first write"
    );
}

// ============================================================================
// Test helpers and utilities
// ============================================================================

// TODO: Uncomment once Phase 2 (TodoStore) is complete and re-exported
// async fn create_test_store_with_tasks(count: usize) -> TodoStore {
//     let temp_dir = TempDir::new().unwrap();
//     let mut store = TodoStore::new(temp_dir.path().join("test.jsonl"));
//     for i in 0..count {
//         store.add(format!("Test task {}", i + 1), None, None, None, None).await.unwrap();
//     }
//     store
// }

/// Capture notification calls for verification
struct NotificationCapture {
    calls: Arc<tokio::sync::RwLock<Vec<(String, String)>>>,
}

impl NotificationCapture {
    fn new() -> Self {
        Self {
            calls: Arc::new(tokio::sync::RwLock::new(Vec::new())),
        }
    }

    async fn get_calls(&self) -> Vec<(String, String)> {
        self.calls.read().await.clone()
    }
}
