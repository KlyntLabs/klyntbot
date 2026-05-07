use klynt_core::registry::builder::ToolKitBuilder;
use klynt_hooks::HookEngine;
use std::sync::Arc;
use storage::{Repos, StoragePool};

#[tokio::test]
async fn tool_kit_builder_carries_hook_engine() {
    let engine = Arc::new(HookEngine::empty());
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repos = Repos::from_pool(&pool);
    let builder = ToolKitBuilder {
        cwd: std::env::current_dir().unwrap(),
        policy: Arc::new(klynt_execpolicy::Policy::empty()),
        privacy: Arc::new(klynt_core::privacy::PrivacyGuard::from_globs(&[]).unwrap()),
        bus: Arc::new(bus::DomainEventBus::new(16)),
        repos,
        non_ui_policy: common::tool_channel::NonUiPolicy::Allow,
        hook_engine: Some(engine.clone()),
        snapshot_repo: None,
        session_key: String::new(),
        history_repo: None,
        mirror_learning_enabled: false,
        mirror_min_approvals: 5,
        mirror_cooldown_seconds: 86400,
        repo_id: String::new(),
    };
    assert!(builder.hook_engine.is_some());
    assert!(Arc::ptr_eq(&builder.hook_engine.unwrap(), &engine));
}
