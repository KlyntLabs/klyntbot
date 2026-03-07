use chrono::Utc;
use feature_session_tracker::repos::SessionTrackerRepos;
use feature_session_tracker::types::*;
use feature_session_tracker::SessionTrackerFeature;
use storage::StoragePool;

async fn setup() -> SessionTrackerRepos {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &SessionTrackerFeature::migrations_static())
        .await
        .unwrap();
    SessionTrackerRepos::new(pool.inner().clone())
}

#[tokio::test]
async fn test_upsert_and_list_sessions() {
    let repos = setup().await;

    let session = TrackedSession {
        session_id: "sess-001".to_string(),
        project_path: "/Users/test/project".to_string(),
        project_name: "project".to_string(),
        jsonl_path: "/home/.claude/projects/test/sess-001.jsonl".to_string(),
        status: SessionStatus::Active,
        first_message_preview: Some("Hello".to_string()),
        message_count: 5,
        git_branch: Some("main".to_string()),
        last_activity: Some(Utc::now()),
        created_at: Utc::now(),
    };

    repos.upsert_session(&session).await.unwrap();

    let sessions = repos.list_sessions().await.unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].session_id, "sess-001");
    assert_eq!(sessions[0].status, SessionStatus::Active);
    assert_eq!(sessions[0].project_name, "project");
}

#[tokio::test]
async fn test_pin_and_unpin_messages() {
    let repos = setup().await;

    let session = TrackedSession {
        session_id: "sess-002".to_string(),
        project_path: "/test".to_string(),
        project_name: "test".to_string(),
        jsonl_path: "/test.jsonl".to_string(),
        status: SessionStatus::Active,
        first_message_preview: None,
        message_count: 0,
        git_branch: None,
        last_activity: Some(Utc::now()),
        created_at: Utc::now(),
    };
    repos.upsert_session(&session).await.unwrap();

    let pin = PinnedMessage {
        id: 0,
        session_id: "sess-002".to_string(),
        message_uuid: "msg-42".to_string(),
        message_content: "Important decision".to_string(),
        message_role: "assistant".to_string(),
        pin_order: 1,
        created_at: Utc::now(),
    };
    repos.pin_message(&pin).await.unwrap();

    let pins = repos.list_pins("sess-002").await.unwrap();
    assert_eq!(pins.len(), 1);
    assert_eq!(pins[0].message_uuid, "msg-42");

    repos.unpin_message(pins[0].id).await.unwrap();
    let pins = repos.list_pins("sess-002").await.unwrap();
    assert!(pins.is_empty());
}

#[tokio::test]
async fn test_brainstorm_conversation_and_messages() {
    let repos = setup().await;

    let session = TrackedSession {
        session_id: "sess-003".to_string(),
        project_path: "/test".to_string(),
        project_name: "test".to_string(),
        jsonl_path: "/test.jsonl".to_string(),
        status: SessionStatus::Active,
        first_message_preview: None,
        message_count: 0,
        git_branch: None,
        last_activity: Some(Utc::now()),
        created_at: Utc::now(),
    };
    repos.upsert_session(&session).await.unwrap();

    let conv = BrainstormConversation {
        id: "conv-001".to_string(),
        session_id: "sess-003".to_string(),
        title: Some("Debug auth".to_string()),
        mode: BrainstormMode::DirectModel,
        model_key: Some("gpt-4o".to_string()),
        agent_profile: None,
        created_at: Utc::now(),
        updated_at: None,
    };
    repos.create_conversation(&conv).await.unwrap();

    let convs = repos.list_conversations("sess-003").await.unwrap();
    assert_eq!(convs.len(), 1);
    assert_eq!(convs[0].title.as_deref(), Some("Debug auth"));

    let msg = BrainstormMessage {
        id: "bmsg-001".to_string(),
        conversation_id: "conv-001".to_string(),
        role: "user".to_string(),
        content: "What's wrong with this auth approach?".to_string(),
        is_result_block: false,
        edited_content: None,
        sent_to_cc: false,
        created_at: Utc::now(),
    };
    repos.add_brainstorm_message(&msg).await.unwrap();

    let messages = repos.list_brainstorm_messages("conv-001").await.unwrap();
    assert_eq!(messages.len(), 1);

    repos.mark_sent_to_cc("bmsg-001").await.unwrap();
    let messages = repos.list_brainstorm_messages("conv-001").await.unwrap();
    assert!(messages[0].sent_to_cc);
}

#[tokio::test]
async fn test_session_status_update() {
    let repos = setup().await;

    let session = TrackedSession {
        session_id: "sess-004".to_string(),
        project_path: "/test".to_string(),
        project_name: "test".to_string(),
        jsonl_path: "/test.jsonl".to_string(),
        status: SessionStatus::Active,
        first_message_preview: None,
        message_count: 0,
        git_branch: None,
        last_activity: Some(Utc::now()),
        created_at: Utc::now(),
    };
    repos.upsert_session(&session).await.unwrap();

    repos
        .update_session_status("sess-004", &SessionStatus::Idle)
        .await
        .unwrap();

    let updated = repos.get_session("sess-004").await.unwrap().unwrap();
    assert_eq!(updated.status, SessionStatus::Idle);
}

#[tokio::test]
async fn test_increment_message_count() {
    let repos = setup().await;

    let session = TrackedSession {
        session_id: "sess-005".to_string(),
        project_path: "/test".to_string(),
        project_name: "test".to_string(),
        jsonl_path: "/test.jsonl".to_string(),
        status: SessionStatus::Active,
        first_message_preview: None,
        message_count: 0,
        git_branch: None,
        last_activity: Some(Utc::now()),
        created_at: Utc::now(),
    };
    repos.upsert_session(&session).await.unwrap();

    repos.increment_message_count("sess-005").await.unwrap();
    repos.increment_message_count("sess-005").await.unwrap();

    let updated = repos.get_session("sess-005").await.unwrap().unwrap();
    assert_eq!(updated.message_count, 2);
}

#[tokio::test]
async fn test_session_summary() {
    let repos = setup().await;

    let session = TrackedSession {
        session_id: "sess-006".to_string(),
        project_path: "/test".to_string(),
        project_name: "test".to_string(),
        jsonl_path: "/test.jsonl".to_string(),
        status: SessionStatus::Active,
        first_message_preview: None,
        message_count: 0,
        git_branch: None,
        last_activity: Some(Utc::now()),
        created_at: Utc::now(),
    };
    repos.upsert_session(&session).await.unwrap();

    let summary = ChunkSummary {
        session_id: "sess-006".to_string(),
        chunk_start: 0,
        chunk_end: 50,
        summary: "Implemented auth module".to_string(),
        files_touched: vec!["src/auth.rs".to_string()],
        key_decisions: vec!["Use JWT tokens".to_string()],
        rolling_summary: "Working on authentication with JWT".to_string(),
    };
    repos.save_summary(&summary).await.unwrap();

    let latest = repos.get_latest_summary("sess-006").await.unwrap();
    assert_eq!(
        latest.as_deref(),
        Some("Working on authentication with JWT")
    );
}
