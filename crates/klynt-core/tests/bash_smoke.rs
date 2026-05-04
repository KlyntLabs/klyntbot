#![cfg(target_os = "macos")]

use bus::DomainEventBus;
use common::tool_channel::NonUiPolicy;
use common::{ChannelName, ChatId};
use config::schema::CodingPermissions;
use klynt_core::approval::{Layer1, PendingApprovalsMap};
use klynt_core::privacy::PrivacyGuard;
use klynt_core::tools::bash::BashTool;
use klynt_execpolicy::Policy;
use std::sync::Arc;
use tokio::sync::mpsc;
use tools_core::{RoutingContext, ToolExecute};

#[tokio::test]
async fn echo_hi_runs_and_emits_sandbox_event() {
    let perms = CodingPermissions {
        allow: vec!["Bash(echo *)".into()],
        ..Default::default()
    };
    let layer1 = Arc::new(Layer1::compile(&perms).unwrap());
    let policy = Arc::new(Policy::empty());
    let privacy = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let pending = Arc::new(PendingApprovalsMap::new());
    let bus = Arc::new(DomainEventBus::new(64));
    let (tx, mut rx) = mpsc::channel::<tools_core::events::ToolEvent>(32);

    let tool = BashTool::new(layer1, policy, privacy, pending, bus, NonUiPolicy::Allow);
    let mut ctx = RoutingContext::new(ChannelName::new("coding"), ChatId::new("test"));
    ctx.event_tx = Some(tx.clone());
    let result = tool
        .execute(
            klynt_core::tools::bash::BashArgs {
                command: "echo hi".into(),
                cwd: Some("/tmp".into()),
                timeout_ms: Some(5000),
            },
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.contains("hi"));

    // ctx owns a clone of `tx` via `ctx.event_tx`; drop it before draining `rx`
    // so the receive loop sees EOF.
    drop(tool);
    drop(ctx);
    drop(tx);
    let mut saw_sandbox = false;
    while let Some(e) = rx.recv().await {
        if matches!(
            e,
            tools_core::events::ToolEvent::SandboxPolicyApplied { .. }
        ) {
            saw_sandbox = true;
        }
    }
    assert!(saw_sandbox, "SandboxPolicyApplied must be emitted");
}

#[tokio::test]
async fn denied_command_returns_error_and_does_not_run() {
    let perms = CodingPermissions {
        deny: vec!["Bash(rm -rf *)".into()],
        allow: vec!["Bash(*)".into()],
        ..Default::default()
    };
    let layer1 = Arc::new(Layer1::compile(&perms).unwrap());
    let policy = Arc::new(Policy::empty());
    let privacy = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let pending = Arc::new(PendingApprovalsMap::new());
    let bus = Arc::new(DomainEventBus::new(64));
    let (tx, _rx) = mpsc::channel::<tools_core::events::ToolEvent>(32);
    let tool = BashTool::new(layer1, policy, privacy, pending, bus, NonUiPolicy::Allow);
    let r = tool
        .execute(
            klynt_core::tools::bash::BashArgs {
                command: "rm -rf /tmp/k2".into(),
                cwd: Some("/tmp".into()),
                timeout_ms: Some(5000),
            },
            &RoutingContext::new(ChannelName::new("coding"), ChatId::new("test")),
        )
        .await;
    assert!(r.is_err());
}
