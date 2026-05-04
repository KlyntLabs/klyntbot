use bus::DomainEventBus;
use common::tool_channel::{Channel, NonUiPolicy};
use config::schema::CodingPermissions;
use klynt_core::approval::{guard::evaluate, GuardCtx, Layer1, PendingApprovalsMap};
use klynt_core::privacy::PrivacyGuard;
use klynt_execpolicy::Policy;
use std::sync::Arc;
use storage::repos::{CodingApprovalHistoryRepo, HistoryEntry};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn scenario_mirror_layer3_auto_approves_after_5_prior_allows() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    let history = CodingApprovalHistoryRepo::new(pool.inner().clone());
    let hash = klynt_core::approval::layer3::args_hash_for_relevance(
        "bash",
        r#"{"command":"git status"}"#,
    );
    for _ in 0..5 {
        history
            .record(HistoryEntry {
                tool: "bash".into(),
                args_hash: hash.clone(),
                repo_id: "test-repo".into(),
                decision: "allow".into(),
                decided_by: "user".into(),
                layer: "ask".into(),
            })
            .await
            .unwrap();
    }

    let perms = CodingPermissions {
        ask: vec!["Bash(*)".into()],
        default_if_no_match: "ask".into(),
        mirror_learning: true,
        ..Default::default()
    };
    let l1 = Layer1::compile(&perms).unwrap();
    let privacy = PrivacyGuard::from_globs(&[]).unwrap();
    let policy = Policy::empty();
    let bus = Arc::new(DomainEventBus::new(64));
    let pending = Arc::new(PendingApprovalsMap::new());
    let (tx, mut rx) = mpsc::channel(32);

    let ctx = GuardCtx {
        layer1: &l1,
        policy: &policy,
        privacy: &privacy,
        pending: &pending,
        event_tx: Some(&tx),
        domain_bus: &bus,
        cancel: CancellationToken::new(),
        request_id: "scenario-1".into(),
        args: Some(serde_json::json!({"command":"git status"})),
        cwd: None,
        channel: Channel::Coding,
        non_ui_policy: NonUiPolicy::Allow,
        history_repo: Some(Arc::new(history.clone())),
        repo_id: "test-repo".into(),
        mirror_learning_enabled: true,
        mirror_min_approvals: 5,
        mirror_cooldown_seconds: 86400,
        now_unix: jiff::Timestamp::now().as_second() + 25 * 3600,
        thread_id: None,
        turn_id: None,
    };

    let decision = evaluate(ctx, "bash", r#"{"command":"git status"}"#).await;
    assert!(decision.allowed(), "expected auto-allow, got {decision:?}");

    // Should emit ApprovalRequested + ApprovalResolved with no user input required
    let req = rx.recv().await.unwrap();
    match req {
        tools_core::events::ToolEvent::ApprovalRequested {
            requires_user_input,
            ..
        } => {
            assert!(
                !requires_user_input,
                "Layer 3 should not require user input"
            );
        }
        other => panic!("expected ApprovalRequested, got {other:?}"),
    }
    let res = rx.recv().await.unwrap();
    match res {
        tools_core::events::ToolEvent::ApprovalResolved { decided_by, .. } => {
            assert_eq!(decided_by, "auto_allow", "Layer 3 should auto-allow");
        }
        other => panic!("expected ApprovalResolved, got {other:?}"),
    }
}
