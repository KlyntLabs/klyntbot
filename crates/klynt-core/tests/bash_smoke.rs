#![cfg(target_os = "macos")]

use bus::DomainEventBus;
use common::tool_channel::NonUiPolicy;
use common::{ChannelName, ChatId};
use klynt_core::privacy::PrivacyGuard;
use klynt_core::tools::bash::BashTool;
use klynt_execpolicy::Policy;
use std::sync::Arc;
use tokio::sync::mpsc;
use tools_core::{RoutingContext, ToolExecute};

#[tokio::test]
async fn echo_hi_runs_and_emits_sandbox_event() {
    let policy = Arc::new(Policy::empty());
    let privacy = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let bus = Arc::new(DomainEventBus::new(64));
    let (tx, mut rx) = mpsc::channel::<tools_core::events::ToolEvent>(32);

    let tool = BashTool::new(
        std::path::PathBuf::from("/tmp"),
        policy,
        privacy,
        bus,
        NonUiPolicy::Allow,
    );
    let mut ctx = RoutingContext::new(ChannelName::new("coding"), ChatId::new("test"));
    ctx.event_tx = Some(tx.clone());
    let result = tool
        .execute(
            klynt_core::tools::bash::BashArgs {
                command: "echo hi".into(),
                cwd: Some("/tmp".into()),
                timeout_ms: Some(5000),
                run_in_background: None,
                description: None,
                silent_completion: None,
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

// ──────────────────────────────────────────────────────────────────────
// cwd-resolution invariants — these guard against regressing the leak
// where `BashTool` previously fell back to `std::env::current_dir()` and
// silently scaffolded files inside the running binary's launch dir.

fn make_tool(base_cwd: std::path::PathBuf) -> BashTool {
    let policy = Arc::new(Policy::empty());
    let privacy = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let bus = Arc::new(DomainEventBus::new(8));
    BashTool::new(base_cwd, policy, privacy, bus, NonUiPolicy::Allow)
}

#[tokio::test]
async fn cwd_none_uses_registry_base() {
    let tmp = tempfile::tempdir().unwrap();
    let base = tmp.path().to_path_buf();
    let tool = make_tool(base.clone());
    let out = tool
        .execute(
            klynt_core::tools::bash::BashArgs {
                command: "pwd".into(),
                cwd: None,
                timeout_ms: Some(5000),
                run_in_background: None,
                description: None,
                silent_completion: None,
            },
            &RoutingContext::new(ChannelName::new("coding"), ChatId::new("test")),
        )
        .await
        .unwrap();
    // Compare canonicalized to neutralise macOS /private/tmp symlink.
    let expected = std::fs::canonicalize(&base).unwrap();
    let actual = std::fs::canonicalize(out.trim()).unwrap();
    assert_eq!(
        actual, expected,
        "bash with cwd=None must run in registry base"
    );
}

#[tokio::test]
async fn cwd_relative_joins_registry_base() {
    let tmp = tempfile::tempdir().unwrap();
    let base = tmp.path().to_path_buf();
    std::fs::create_dir(base.join("sub")).unwrap();
    let tool = make_tool(base.clone());
    let out = tool
        .execute(
            klynt_core::tools::bash::BashArgs {
                command: "pwd".into(),
                cwd: Some("sub".into()),
                timeout_ms: Some(5000),
                run_in_background: None,
                description: None,
                silent_completion: None,
            },
            &RoutingContext::new(ChannelName::new("coding"), ChatId::new("test")),
        )
        .await
        .unwrap();
    let expected = std::fs::canonicalize(base.join("sub")).unwrap();
    let actual = std::fs::canonicalize(out.trim()).unwrap();
    assert_eq!(actual, expected, "relative cwd must join registry base");
}

#[tokio::test]
async fn cwd_absolute_wins_over_registry_base() {
    let tmp = tempfile::tempdir().unwrap();
    let base = tmp.path().to_path_buf();
    let other = tempfile::tempdir().unwrap();
    let other_path = other.path().to_path_buf();
    let tool = make_tool(base.clone());
    let out = tool
        .execute(
            klynt_core::tools::bash::BashArgs {
                command: "pwd".into(),
                cwd: Some(other_path.to_string_lossy().into_owned()),
                timeout_ms: Some(5000),
                run_in_background: None,
                description: None,
                silent_completion: None,
            },
            &RoutingContext::new(ChannelName::new("coding"), ChatId::new("test")),
        )
        .await
        .unwrap();
    let expected = std::fs::canonicalize(&other_path).unwrap();
    let actual = std::fs::canonicalize(out.trim()).unwrap();
    assert_eq!(actual, expected, "absolute cwd must override registry base");
}

#[tokio::test]
async fn denied_command_returns_error_and_does_not_run() {
    let policy = Arc::new(Policy::empty());
    let privacy = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let bus = Arc::new(DomainEventBus::new(64));
    let (_tx, _rx) = mpsc::channel::<tools_core::events::ToolEvent>(32);
    let tool = BashTool::new(
        std::path::PathBuf::from("/tmp"),
        policy,
        privacy,
        bus,
        NonUiPolicy::Allow,
    );
    let r = tool
        .execute(
            klynt_core::tools::bash::BashArgs {
                command: "rm -rf /tmp/k2".into(),
                cwd: Some("/tmp".into()),
                timeout_ms: Some(5000),
                run_in_background: None,
                description: None,
                silent_completion: None,
            },
            &RoutingContext::new(ChannelName::new("coding"), ChatId::new("test")),
        )
        .await;
    assert!(r.is_err());
}
