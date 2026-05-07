use bus::DomainEventBus;
use common::tool_channel::NonUiPolicy;
use klynt_core::privacy::PrivacyGuard;
use klynt_core::tools::bash::BashTool;
use klynt_execpolicy::Policy;
use std::sync::Arc;
use tools_core::Tool;

#[test]
fn bash_tool_metadata() {
    let policy = Arc::new(Policy::empty());
    let privacy = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let bus = Arc::new(DomainEventBus::new(1));

    let t = BashTool::new(std::path::PathBuf::from("/tmp"), policy, privacy, bus, NonUiPolicy::Allow);
    assert_eq!(t.name(), "bash");
    assert!(!t.description().is_empty());
    let schema = t.parameters();
    assert!(schema["properties"]["command"].is_object());
    assert!(!t.is_concurrency_safe(&serde_json::json!({"command":"echo"})));
}
