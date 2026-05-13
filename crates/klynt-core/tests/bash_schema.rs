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

    let t = BashTool::new(
        std::path::PathBuf::from("/tmp"),
        policy,
        privacy,
        bus,
        NonUiPolicy::Allow,
    );
    assert_eq!(t.name(), "bash");
    assert!(!t.description().is_empty());
    let schema = t.parameters();
    assert!(schema["properties"]["command"].is_object());
    assert!(!t.is_concurrency_safe(&serde_json::json!({"command":"echo"})));
}

#[test]
fn bash_args_includes_tty_fields() {
    use klynt_core::tools::bash::BashArgs;
    let schema = <BashArgs as tools_core::ToolParams>::schema();
    let s = serde_json::to_string(&schema).unwrap();
    assert!(s.contains("tty"), "schema missing tty: {s}");
    assert!(s.contains("tty_rows"), "schema missing tty_rows: {s}");
    assert!(s.contains("tty_cols"), "schema missing tty_cols: {s}");
}
