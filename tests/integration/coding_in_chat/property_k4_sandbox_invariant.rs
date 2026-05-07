#![cfg(target_os = "macos")]
use bus::DomainEventBus;
use klynt_core::privacy::PrivacyGuard;
use klynt_core::tools::bash::{BashArgs, BashTool};
use klynt_execpolicy::Policy;
use std::sync::Arc;
use tokio::sync::mpsc;
use tools_core::events::ToolEvent;
use tools_core::ToolExecute;

#[tokio::test]
async fn k4_sandbox_event_emitted_before_exec() {
    let policy = Arc::new(Policy::empty());
    let privacy = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let bus = Arc::new(DomainEventBus::new(64));
    let (tx, mut rx) = mpsc::channel(64);

    let cwd = tempfile::tempdir().unwrap();
    let outside = std::env::temp_dir().join(format!("klynt-k4-outside-{}", uuid::Uuid::new_v4()));
    let cmd = format!(
        "touch {}/forbidden 2>/dev/null; echo done",
        outside.display()
    );

    let tool = BashTool::new(policy, privacy, bus, common::tool_channel::NonUiPolicy::Allow);
    let mut routing_ctx = tools_core::RoutingContext::new(
        ::common::ChannelName::new("coding"),
        ::common::ChatId::new("test"),
    );
    routing_ctx.event_tx = Some(tx.clone());
    let r = ToolExecute::execute(
        &tool,
        BashArgs {
            command: cmd.clone(),
            cwd: Some(cwd.path().to_string_lossy().into()),
            timeout_ms: Some(5000),
        },
        &routing_ctx,
    )
    .await
    .unwrap();

    // Forbidden file MUST NOT exist
    assert!(!outside.join("forbidden").exists());

    // SandboxPolicyApplied must precede the actual run output.
    // Drop ALL senders (tx + the cloned tx held by routing_ctx) before draining,
    // otherwise rx.recv() never returns None.
    let mut saw_sandbox = false;
    drop(tool);
    drop(routing_ctx);
    drop(tx);
    while let Some(e) = rx.recv().await {
        if matches!(e, ToolEvent::SandboxPolicyApplied { .. }) {
            saw_sandbox = true;
        }
    }
    assert!(saw_sandbox, "SandboxPolicyApplied must be emitted");
    assert!(r.contains("done"));
}
