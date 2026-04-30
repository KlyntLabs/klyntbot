#![cfg(target_os = "macos")]
use agent::events::AgentEvent;
use bus::DomainEventBus;
use config::schema::CodingPermissions;
use klynt_core::approval::{Layer1, PendingApprovalsMap};
use klynt_core::privacy::PrivacyGuard;
use klynt_core::tools::bash::{BashArgs, BashTool};
use klynt_execpolicy::Policy;
use std::sync::Arc;
use tokio::sync::mpsc;
use tools_core::ToolExecute;

#[tokio::test]
async fn bash_happy_path() {
    let perms = CodingPermissions {
        allow: vec!["Bash(echo *)".into()],
        ..Default::default()
    };
    let layer1 = Arc::new(Layer1::compile(&perms).unwrap());
    let policy = Arc::new(Policy::empty());
    let privacy = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let pending = Arc::new(PendingApprovalsMap::new());
    let bus = Arc::new(DomainEventBus::new(256));
    let (tx, mut rx) = mpsc::channel(256);

    let tool = BashTool::new(layer1, policy, privacy, pending, tx, bus);
    let result = ToolExecute::execute(
        &tool,
        BashArgs {
            command: "echo hi".into(),
            cwd: Some("/tmp".into()),
            timeout_ms: Some(5000),
        },
        &tools_core::RoutingContext::new(
            ::common::ChannelName::new("coding"),
            ::common::ChatId::new("test"),
        ),
    )
    .await
    .unwrap();

    assert!(result.contains("hi"));

    drop(tool);

    let mut saw_request = false;
    let mut saw_resolved = false;
    let mut saw_sandbox = false;
    while let Some(e) = rx.recv().await {
        match e {
            AgentEvent::ApprovalRequested {
                ref tool,
                requires_user_input,
                ..
            } if tool == "bash" && !requires_user_input => saw_request = true,
            AgentEvent::ApprovalResolved { .. } if saw_request => saw_resolved = true,
            AgentEvent::SandboxPolicyApplied { .. } if saw_resolved => saw_sandbox = true,
            _ => {}
        }
    }
    assert!(
        saw_request && saw_resolved && saw_sandbox,
        "expected ApprovalRequested → ApprovalResolved → SandboxPolicyApplied"
    );
}
