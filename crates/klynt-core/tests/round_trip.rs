use klynt_core::approval::decision::{ApprovalDecision, ApprovalLayer};
use klynt_core::approval::round_trip::{await_decision, PendingApprovalsMap};
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn user_approves_resolves() {
    let map = Arc::new(PendingApprovalsMap::new());
    let req_id = "req-1".to_string();
    let token = CancellationToken::new();
    let map2 = map.clone();
    let req2 = req_id.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(20)).await;
        map2.resolve(
            &req2,
            ApprovalDecision::Auto {
                allowed: true,
                layer: ApprovalLayer::Layer1Declarative,
                reason: "user clicked allow once".into(),
                rule_matched: None,
            },
        );
    });
    let d = await_decision(&map, &req_id, token, Duration::from_secs(2)).await;
    assert!(d.allowed());
}

#[tokio::test]
async fn cancellation_resolves_as_cancelled() {
    let map = Arc::new(PendingApprovalsMap::new());
    let token = CancellationToken::new();
    let token2 = token.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(20)).await;
        token2.cancel();
    });
    let d = await_decision(&map, "rid", token, Duration::from_secs(2)).await;
    assert!(matches!(d, ApprovalDecision::Cancelled));
}

#[tokio::test]
async fn timeout_resolves_as_timeout() {
    let map = Arc::new(PendingApprovalsMap::new());
    let token = CancellationToken::new();
    let d = await_decision(&map, "rid", token, Duration::from_millis(50)).await;
    assert!(matches!(d, ApprovalDecision::TimedOut));
}

#[tokio::test]
async fn unknown_request_id_resolve_is_noop() {
    let map = PendingApprovalsMap::new();
    map.resolve("nonexistent", ApprovalDecision::Cancelled);
}
