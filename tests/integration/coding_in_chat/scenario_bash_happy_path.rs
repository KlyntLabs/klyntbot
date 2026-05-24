#![cfg(target_os = "macos")]
use bus::DomainEventBus;
use klynt_core::privacy::PrivacyGuard;
use klynt_core::tools::bash::{BashArgs, BashTool};
use klynt_execpolicy::Policy;
use std::sync::Arc;
use tokio::sync::mpsc;
use tools_core::{events::ToolEvent, FullCtx, ToolExecute};

#[tokio::test]
async fn bash_happy_path() {
    let policy = Arc::new(Policy::empty());
    let privacy = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let bus = Arc::new(DomainEventBus::new(256));
    let (tx, mut rx) = mpsc::channel(256);

    let tool = BashTool::new(
        std::path::PathBuf::from("/tmp"),
        policy,
        privacy,
        bus,
        common::tool_channel::NonUiPolicy::Allow,
    );
    let mut routing_ctx = tools_core::RoutingContext::new(
        ::common::ChannelName::new("coding"),
        ::common::ChatId::new("test"),
    );
    routing_ctx.event_tx = Some(tx);
    let result = ToolExecute::execute(
        &tool,
        BashArgs {
            command: "echo hi".into(),
            cwd: Some("/tmp".into()),
            timeout_ms: Some(5000),
            run_in_background: None,
            description: None,
            silent_completion: None,
            tty: None,
            tty_rows: None,
            tty_cols: None,
        },
        FullCtx(&routing_ctx),
    )
    .await
    .unwrap();

    assert!(result.contains("hi"));

    drop(tool);
    drop(routing_ctx); // close the sender so recv drains then returns None

    let mut saw_sandbox = false;
    while let Some(e) = rx.recv().await {
        if matches!(e, ToolEvent::SandboxPolicyApplied { .. }) {
            saw_sandbox = true;
        }
    }
    assert!(
        saw_sandbox,
        "expected SandboxPolicyApplied; got sandbox={saw_sandbox}"
    );
}
