use bus::DomainEventBus;
use klynt_core::approval::{Layer1, PendingApprovalsMap};
use klynt_core::privacy::PrivacyGuard;
use klynt_core::tools::bash::BashTool;
use klynt_execpolicy::Policy;
use std::sync::Arc;
use tokio::sync::mpsc;
use tools_core::Tool;
use common::tool_channel::NonUiPolicy;

#[test]
fn bash_tool_metadata() {
    let layer1 = Arc::new(Layer1::compile(&config::schema::CodingPermissions::default()).unwrap());
    let policy = Arc::new(Policy::empty());
    let privacy = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let pending = Arc::new(PendingApprovalsMap::new());
    let (tx, _rx) = mpsc::channel::<tools_core::events::ToolEvent>(1);
    let bus = Arc::new(DomainEventBus::new(1));

    let t = BashTool::new(layer1, policy, privacy, pending, bus, NonUiPolicy::Allow);
    assert_eq!(t.name(), "bash");
    assert!(!t.description().is_empty());
    let schema = t.parameters();
    assert!(schema["properties"]["command"].is_object());
    assert!(!t.is_concurrency_safe(&serde_json::json!({"command":"echo"})));
}
