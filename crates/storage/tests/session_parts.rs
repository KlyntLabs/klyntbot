use serde_json::json;
use storage::messages::parts::MessagePart;
use storage::repos::SessionRepo;
use storage::StoragePool;

#[tokio::test]
async fn add_message_with_parts_round_trip() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repo = SessionRepo::new(pool.inner().clone());
    let sk = "test-session";
    repo.upsert_session(sk, &json!({})).await.unwrap();

    let parts = vec![
        MessagePart::Text {
            text: "hello".into(),
        },
        MessagePart::ToolCall {
            call_id: "c1".into(),
            name: "bash".into(),
            args: json!({"cmd": "ls"}),
        },
    ];
    let msg_id = uuid::Uuid::new_v4();
    repo.add_message_with_parts(sk, msg_id, "assistant", &parts, Some("turn-1"), None)
        .await
        .unwrap();

    let fetched = repo.get_messages_parts(sk, 100).await.unwrap();
    assert_eq!(fetched.len(), 1);
    assert_eq!(fetched[0].parts.len(), 2);
    match &fetched[0].parts[0] {
        MessagePart::Text { text } => assert_eq!(text, "hello"),
        other => panic!("expected Text, got {other:?}"),
    }
    assert_eq!(fetched[0].turn_id.as_deref(), Some("turn-1"));
}

#[tokio::test]
async fn get_messages_parts_legacy_fallback() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repo = SessionRepo::new(pool.inner().clone());
    let sk = "legacy-session";
    repo.upsert_session(sk, &json!({})).await.unwrap();

    let msg_id = uuid::Uuid::new_v4();
    repo.add_message(sk, msg_id, "user", "legacy content", None, None, None)
        .await
        .unwrap();

    let fetched = repo.get_messages_parts(sk, 100).await.unwrap();
    assert_eq!(fetched.len(), 1);
    match &fetched[0].parts[0] {
        MessagePart::Text { text } => assert_eq!(text, "legacy content"),
        other => panic!("expected Text fallback, got {other:?}"),
    }
}

#[tokio::test]
async fn set_workspace_id_and_ephemeral() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repo = SessionRepo::new(pool.inner().clone());
    let sk = "ws-test-session";
    repo.upsert_session(sk, &json!({})).await.unwrap();

    repo.set_ephemeral(sk, true).await.unwrap();

    let session = repo.get_session(sk).await.unwrap();
    assert_eq!(session.ephemeral, 1);
    // Note: set_workspace_id requires a valid workspace FK, tested in integration
}
