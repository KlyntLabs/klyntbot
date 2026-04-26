use coding_ingest::event::{AgentEvent, AgentEventV1, AgentSource, EventKind};
use coding_ingest::store::IngestEventLogRepo;
use jiff::Timestamp;
use std::path::PathBuf;
use storage::StoragePool;
use uuid::Uuid;

fn evt(session: &str, turn: Option<&str>, kind: EventKind) -> AgentEvent {
    AgentEvent::V1(AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::ClaudeCode,
        session_id: session.into(),
        turn_id: turn.map(str::to_string),
        cwd: PathBuf::from("/tmp"),
        repo: None,
        occurred_at: Timestamp::now(),
        kind,
    })
}

async fn prepared() -> (StoragePool, IngestEventLogRepo) {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();
    let repo = IngestEventLogRepo::new(pool.inner().clone());
    (pool, repo)
}

#[tokio::test]
async fn fetch_turn_returns_only_events_for_session_turn() {
    let (_pool, repo) = prepared().await;
    let up = EventKind::UserPrompt {
        text: "hi".into(),
        attachments: vec![],
    };
    repo.insert(&evt("s1", Some("t1"), up.clone()))
        .await
        .unwrap();
    repo.insert(&evt("s1", Some("t1"), up.clone()))
        .await
        .unwrap();
    repo.insert(&evt("s1", Some("t2"), up.clone()))
        .await
        .unwrap();
    repo.insert(&evt("s2", Some("t1"), up.clone()))
        .await
        .unwrap();

    let rows = repo.fetch_turn("s1", Some("t1")).await.unwrap();
    assert_eq!(rows.len(), 2);
    for r in &rows {
        assert_eq!(r.session_id, "s1");
        assert_eq!(r.turn_id.as_deref(), Some("t1"));
    }
}

#[tokio::test]
async fn fetch_turn_null_turn_id() {
    let (_pool, repo) = prepared().await;
    repo.insert(&evt(
        "s1",
        None,
        EventKind::SessionEnd { reason: "x".into() },
    ))
    .await
    .unwrap();
    let rows = repo.fetch_turn("s1", None).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert!(rows[0].turn_id.is_none());
}

#[tokio::test]
async fn mark_processing_transitions_atomically() {
    let (_pool, repo) = prepared().await;
    let up = EventKind::UserPrompt {
        text: "hi".into(),
        attachments: vec![],
    };
    repo.insert(&evt("s1", Some("t1"), up.clone()))
        .await
        .unwrap();
    repo.insert(&evt("s1", Some("t1"), up.clone()))
        .await
        .unwrap();

    let claimed = repo.mark_processing("s1", Some("t1")).await.unwrap();
    assert_eq!(claimed, 2);
    // Second claim finds nothing — idempotent.
    let claimed = repo.mark_processing("s1", Some("t1")).await.unwrap();
    assert_eq!(claimed, 0);
}

#[tokio::test]
async fn mark_processed_completes_turn() {
    let (_pool, repo) = prepared().await;
    let up = EventKind::UserPrompt {
        text: "hi".into(),
        attachments: vec![],
    };
    repo.insert(&evt("s1", Some("t1"), up)).await.unwrap();
    repo.mark_processing("s1", Some("t1")).await.unwrap();
    let rows = repo.fetch_turn("s1", Some("t1")).await.unwrap();
    repo.mark_processed_iter(rows.iter().map(|r| r.id.as_str()))
        .await
        .unwrap();
    assert_eq!(repo.count_unprocessed().await.unwrap(), 0);
}
