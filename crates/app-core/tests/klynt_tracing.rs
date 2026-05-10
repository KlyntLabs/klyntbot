//! Integration test for the Klynt tracing provider against an in-memory pool.

use app_core::tracing::provider::TracingProvider;
use app_core::tracing::providers::klynt::KlyntTracingProvider;
use app_core::tracing::types::{HeaderChip, Scope, SessionTab};
use common::SessionMode;
use std::path::PathBuf;
use storage::messages::parts::MessagePart;
use storage::repos::Repos;
use storage::StoragePool;

async fn provider_with_seeded_session() -> (KlyntTracingProvider, Repos) {
    let pool = StoragePool::connect_in_memory().await.expect("pool");
    let repos = Repos::from_pool(&pool);
    repos
        .sessions
        .upsert_session_with_mode(
            "coding:demo",
            SessionMode::Coding,
            &serde_json::json!({"title": "Demo"}),
        )
        .await
        .unwrap();
    repos
        .sessions
        .add_message_with_parts(
            "coding:demo",
            uuid::Uuid::new_v4(),
            "user",
            &[MessagePart::Text {
                text: "hello".into(),
            }],
            Some("t1"),
            None,
        )
        .await
        .unwrap();
    repos
        .sessions
        .add_message_with_parts(
            "coding:demo",
            uuid::Uuid::new_v4(),
            "assistant",
            &[
                MessagePart::Text { text: "ok".into() },
                MessagePart::ToolCall {
                    call_id: "c1".into(),
                    name: "bash".into(),
                    args: serde_json::json!({"cmd": "ls"}),
                },
            ],
            Some("t1"),
            None,
        )
        .await
        .unwrap();
    let provider = KlyntTracingProvider::new(repos.clone(), PathBuf::from("/tmp/klyntbot-test"));
    (provider, repos)
}

#[tokio::test]
async fn provider_id_and_display_name() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let provider = KlyntTracingProvider::new(Repos::from_pool(&pool), PathBuf::from("/tmp"));
    assert_eq!(provider.id(), "klynt");
    assert_eq!(provider.display_name(), "Klynt");
}

#[tokio::test]
async fn supported_tabs_includes_wire_context_state_agents() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let provider = KlyntTracingProvider::new(Repos::from_pool(&pool), PathBuf::from("/tmp"));
    let tabs = provider.supported_tabs();
    assert!(tabs.contains(&SessionTab::Wire));
    assert!(tabs.contains(&SessionTab::Context));
    assert!(tabs.contains(&SessionTab::State));
    assert!(tabs.contains(&SessionTab::Agents));
}

#[tokio::test]
async fn header_layout_includes_compactions() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let provider = KlyntTracingProvider::new(Repos::from_pool(&pool), PathBuf::from("/tmp"));
    assert!(provider.header_layout().contains(&HeaderChip::Compactions));
    assert!(provider.header_layout().contains(&HeaderChip::Tokens));
}

#[tokio::test]
async fn list_sessions_finds_seeded_session() {
    let (provider, _) = provider_with_seeded_session().await;
    let sessions = provider.list_sessions().await.unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].session_id, "coding:demo");
    assert_eq!(sessions[0].provider_id, "klynt");
    assert_eq!(sessions[0].custom_title.as_deref(), Some("Demo"));
}

#[tokio::test]
async fn load_session_returns_three_events_in_part_order() {
    let (provider, _) = provider_with_seeded_session().await;
    let detail = provider
        .load_session("coding:demo", Scope::Main)
        .await
        .unwrap();
    assert_eq!(detail.events.len(), 3);
    assert_eq!(detail.events[0].raw_kind, "UserMessage");
    assert_eq!(detail.events[1].raw_kind, "ContentChunk");
    assert_eq!(detail.events[2].raw_kind, "ToolCall");
}

#[tokio::test]
async fn import_from_file_returns_unsupported() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let provider = KlyntTracingProvider::new(Repos::from_pool(&pool), PathBuf::from("/tmp"));
    let path = std::path::Path::new("/nonexistent");
    assert!(provider.import_from_file(path).await.is_err());
}

#[tokio::test]
async fn open_dir_returns_data_dir() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let provider = KlyntTracingProvider::new(Repos::from_pool(&pool), PathBuf::from("/tmp/dd"));
    let dir = provider.open_dir("any").await.unwrap();
    assert_eq!(dir, PathBuf::from("/tmp/dd"));
}
