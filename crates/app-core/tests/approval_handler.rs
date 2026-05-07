use app_core::coding::approval_handler::{respond_approval, AppApprovalDecision};
use app_core::desktop_approval_channel::DesktopApprovalChannel;
use approval::{ApprovalChannel, ApprovalClass, ApprovalContext, ApprovalDecision, ApprovalGrantsRepo, ApprovalRequest, ApprovalScope, ChannelKind};
use std::sync::Arc;
use storage::StoragePool;

fn ctx() -> ApprovalContext {
    ApprovalContext {
        mode: common::SessionMode::Coding,
        channel: ChannelKind::Desktop,
        session_id: "sess-1".into(),
        user_id: None,
        cwd: std::path::PathBuf::from("."),
    }
}

fn make_req(tool_name: &str, args: serde_json::Value) -> ApprovalRequest {
    ApprovalRequest {
        tool_name: tool_name.into(),
        action: None,
        args,
        class: ApprovalClass::Destructive,
        scope: ApprovalScope::ToolAction,
        ctx: ctx(),
        preview: None,
        suggested_grant: None,
    }
}

#[tokio::test]
async fn approve_once_unblocks_tool() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let grants_repo = Arc::new(ApprovalGrantsRepo::new(pool));
    let channel = Arc::new(DesktopApprovalChannel::new(Arc::new(app_core::events::NoopEmitter)));

    // Start an approval request on the channel
    let chan_clone = channel.clone();
    let req = make_req("bash", serde_json::json!({"command": "echo hi"}));
    let fut = tokio::spawn(async move { chan_clone.request(req).await });

    // Give the channel a moment to register the pending request
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let pending_ids = channel.pending_ids();
    assert_eq!(pending_ids.len(), 1);
    let request_id = pending_ids[0].clone();

    let result = respond_approval(
        channel.clone(),
        grants_repo.clone(),
        None,
        &request_id,
        AppApprovalDecision::AllowOnce,
    )
    .await;
    assert!(result.is_ok());

    // The gate future should resolve to Once
    let decision = fut.await.unwrap();
    assert!(matches!(decision, ApprovalDecision::Once));

    // No Forever grant should have been persisted
    let grant = grants_repo
        .find(ApprovalClass::Destructive, "bash", None, None, None)
        .await
        .unwrap();
    assert!(grant.is_none());
}

#[tokio::test]
async fn approve_always_persists_grant() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let grants_repo = Arc::new(ApprovalGrantsRepo::new(pool));
    let channel = Arc::new(DesktopApprovalChannel::new(Arc::new(app_core::events::NoopEmitter)));

    let chan_clone = channel.clone();
    let req = make_req("edit", serde_json::json!({"path": "src/main.rs", "content": "fn main() {}"}));
    let fut = tokio::spawn(async move { chan_clone.request(req).await });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let pending_ids = channel.pending_ids();
    assert_eq!(pending_ids.len(), 1);
    let request_id = pending_ids[0].clone();

    let result = respond_approval(
        channel.clone(),
        grants_repo.clone(),
        None,
        &request_id,
        AppApprovalDecision::AllowAlways { rule: None },
    )
    .await;
    assert!(result.is_ok());

    let decision = fut.await.unwrap();
    assert!(matches!(decision, ApprovalDecision::Forever));

    // A Forever grant should have been persisted
    let grant = grants_repo
        .find(ApprovalClass::Destructive, "edit", None, Some("src/main.rs"), None)
        .await
        .unwrap();
    assert!(grant.is_some());
    let grant = grant.unwrap();
    assert_eq!(grant.tool_name, "edit");
    assert_eq!(grant.resource_key.as_deref(), Some("src/main.rs"));
    assert!(matches!(grant.lifetime, approval::ApprovalLifetime::Forever));
}

#[tokio::test]
async fn deny_returns_decline() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let grants_repo = Arc::new(ApprovalGrantsRepo::new(pool));
    let channel = Arc::new(DesktopApprovalChannel::new(Arc::new(app_core::events::NoopEmitter)));

    let chan_clone = channel.clone();
    let req = make_req("bash", serde_json::json!({"command": "rm -rf /"}));
    let fut = tokio::spawn(async move { chan_clone.request(req).await });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let pending_ids = channel.pending_ids();
    assert_eq!(pending_ids.len(), 1);
    let request_id = pending_ids[0].clone();

    let result = respond_approval(
        channel.clone(),
        grants_repo.clone(),
        None,
        &request_id,
        AppApprovalDecision::Deny,
    )
    .await;
    assert!(result.is_ok());

    let decision = fut.await.unwrap();
    assert!(matches!(decision, ApprovalDecision::Decline { .. }));

    // No grant should have been persisted
    let grant = grants_repo
        .find(ApprovalClass::Destructive, "bash", None, None, None)
        .await
        .unwrap();
    assert!(grant.is_none());
}
