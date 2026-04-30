use agent::events::AgentEvent;
use bus::DomainEventBus;
use klynt_core::tools::plan_mode::{run_enter_for_test, run_exit_for_test};
use std::sync::Arc;
use storage::{Repos, StoragePool};
use tokio::sync::mpsc;

#[tokio::test]
async fn enter_sets_approval_mode_plan_and_emits_event() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repos = Repos::from_pool(&pool);
    let key = repos.sessions.upsert_session("u1", &serde_json::json!({})).await.unwrap().key;
    let bus = Arc::new(DomainEventBus::new(64));
    let (tx, mut rx) = mpsc::channel(32);
    run_enter_for_test(&repos, &key, tx.clone(), bus.clone()).await.unwrap();
    let row = repos.sessions.find_by_key(&key).await.unwrap().unwrap();
    assert_eq!(row.approval_mode, "plan");
    drop(tx);
    let mut saw = false;
    while let Some(e) = rx.recv().await {
        if let AgentEvent::PlanModeChanged { active: true, .. } = e { saw = true; }
    }
    assert!(saw);
}

#[tokio::test]
async fn exit_sets_approval_mode_default_and_emits_event() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repos = Repos::from_pool(&pool);
    let key = repos.sessions.upsert_session("u1", &serde_json::json!({})).await.unwrap().key;
    repos.sessions.update_approval_mode(&key, "plan").await.unwrap();
    let bus = Arc::new(DomainEventBus::new(64));
    let (tx, mut rx) = mpsc::channel(32);
    run_exit_for_test(&repos, &key, tx.clone(), bus.clone()).await.unwrap();
    let row = repos.sessions.find_by_key(&key).await.unwrap().unwrap();
    assert_eq!(row.approval_mode, "default");
    drop(tx);
    let mut saw = false;
    while let Some(e) = rx.recv().await {
        if let AgentEvent::PlanModeChanged { active: false, .. } = e { saw = true; }
    }
    assert!(saw);
}
