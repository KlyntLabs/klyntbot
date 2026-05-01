use std::sync::Arc;
use klynt_core::registry::builder::ToolKitBuilder;
use klynt_hooks::HookEngine;
use storage::{Repos, StoragePool};

#[tokio::test]
async fn tool_kit_builder_carries_hook_engine() {
    let engine = Arc::new(HookEngine::empty());
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repos = Repos::from_pool(&pool);
    let builder = ToolKitBuilder {
        cwd: std::env::current_dir().unwrap(),
        layer1: Arc::new(klynt_core::approval::Layer1::compile(&config::schema::CodingPermissions::default()).unwrap()),
        policy: Arc::new(klynt_execpolicy::Policy::empty()),
        privacy: Arc::new(klynt_core::privacy::PrivacyGuard::from_globs(&[]).unwrap()),
        pending: Arc::new(klynt_core::approval::PendingApprovalsMap::default()),
        bus: Arc::new(bus::DomainEventBus::new(16)),
        repos,
        host_cache: Arc::new(klynt_core::approval::HostApprovalCache::default()),
        non_ui_policy: common::tool_channel::NonUiPolicy::Allow,
        hook_engine: Some(engine.clone()),
    };
    assert!(builder.hook_engine.is_some());
    assert!(Arc::ptr_eq(&builder.hook_engine.unwrap(), &engine));
}
