use bus::DomainEventBus;
use common::tool_channel::{Channel, NonUiPolicy};
use config::schema::CodingPermissions;
use klynt_core::approval::{
    decision::{ApprovalDecision, ApprovalLayer},
    guard::{evaluate, GuardCtx, APPROVAL_TIMEOUT},
    round_trip::PendingApprovalsMap,
    Layer1,
};
use klynt_core::privacy::PrivacyGuard;
use klynt_execpolicy::Policy;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tools_core::events::ToolEvent;

fn guard_ctx_with_history<'a>(
    layer1: &'a Layer1,
    policy: &'a Policy,
    privacy: &'a PrivacyGuard,
    pending: &'a Arc<PendingApprovalsMap>,
    event_tx: Option<&'a mpsc::Sender<ToolEvent>>,
    bus: &'a Arc<DomainEventBus>,
    history_repo: Option<Arc<storage::repos::CodingApprovalHistoryRepo>>,
    mirror_learning: bool,
    now_offset_h: i64,
) -> GuardCtx<'a> {
    GuardCtx {
        layer1,
        policy,
        privacy,
        pending,
        event_tx,
        domain_bus: bus,
        cancel: CancellationToken::new(),
        request_id: "test-mirror".into(),
        args: None,
        cwd: None,
        channel: Channel::Coding,
        non_ui_policy: NonUiPolicy::Allow,
        history_repo,
        repo_id: "test-repo".into(),
        mirror_learning_enabled: mirror_learning,
        mirror_min_approvals: 5,
        mirror_cooldown_seconds: 86400,
        now_unix: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
            + now_offset_h * 3600,
        thread_id: None,
        turn_id: None,
    }
}

#[tokio::test]
async fn privacy_blocks_first() {
    let perms = CodingPermissions {
        allow: vec!["Bash(*)".into()],
        ..Default::default()
    };
    let l1 = Layer1::compile(&perms).unwrap();
    let privacy = PrivacyGuard::from_globs(&["**/.env"]).unwrap();
    let policy = Policy::empty();
    let bus = Arc::new(DomainEventBus::new(64));
    let (tx, mut rx) = mpsc::channel::<ToolEvent>(32);
    let pending = Arc::new(PendingApprovalsMap::new());

    let ctx = GuardCtx {
        layer1: &l1,
        policy: &policy,
        privacy: &privacy,
        pending: &pending,
        event_tx: Some(&tx),
        domain_bus: &bus,
        cancel: CancellationToken::new(),
        request_id: "r1".into(),
        args: None,
        cwd: None,
        channel: Channel::Coding,
        non_ui_policy: NonUiPolicy::Allow,
        history_repo: None,
        repo_id: String::new(),
        mirror_learning_enabled: false,
        mirror_min_approvals: 5,
        mirror_cooldown_seconds: 86400,
        now_unix: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64,
        thread_id: None,
        turn_id: None,
    };
    let d = evaluate(ctx, "bash", "cat .env").await;
    assert!(matches!(d, ApprovalDecision::PrivacyDenied { .. }));
    let evt = rx.recv().await.unwrap();
    assert!(matches!(evt, ToolEvent::ApprovalRequested { .. }));
    let resolved = rx.recv().await.unwrap();
    assert!(matches!(resolved, ToolEvent::ApprovalResolved { .. }));
}

#[tokio::test]
async fn auto_allow_emits_pair_no_user_input() {
    let perms = CodingPermissions {
        allow: vec!["Bash(echo *)".into()],
        default_if_no_match: "ask".into(),
        mirror_learning: false,
        ..Default::default()
    };
    let l1 = Layer1::compile(&perms).unwrap();
    let privacy = PrivacyGuard::from_globs(&[]).unwrap();
    let policy = Policy::empty();
    let bus = Arc::new(DomainEventBus::new(64));
    let (tx, mut rx) = mpsc::channel::<ToolEvent>(32);
    let pending = Arc::new(PendingApprovalsMap::new());

    let ctx = GuardCtx {
        layer1: &l1,
        policy: &policy,
        privacy: &privacy,
        pending: &pending,
        event_tx: Some(&tx),
        domain_bus: &bus,
        cancel: CancellationToken::new(),
        request_id: "r2".into(),
        args: None,
        cwd: None,
        channel: Channel::Coding,
        non_ui_policy: NonUiPolicy::Allow,
        history_repo: None,
        repo_id: String::new(),
        mirror_learning_enabled: false,
        mirror_min_approvals: 5,
        mirror_cooldown_seconds: 86400,
        now_unix: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64,
        thread_id: None,
        turn_id: None,
    };
    let d = evaluate(ctx, "bash", "echo hi").await;
    assert!(d.allowed());
    let req = rx.recv().await.unwrap();
    if let ToolEvent::ApprovalRequested {
        requires_user_input,
        ..
    } = req
    {
        assert!(!requires_user_input);
    } else {
        panic!("expected ApprovalRequested");
    }
    assert!(matches!(
        rx.recv().await.unwrap(),
        ToolEvent::ApprovalResolved { .. }
    ));
}

#[tokio::test]
async fn ask_path_awaits_user_decision() {
    let perms = CodingPermissions {
        ask: vec!["Bash(*)".into()],
        default_if_no_match: "ask".into(),
        ..Default::default()
    };
    let l1 = Layer1::compile(&perms).unwrap();
    let privacy = PrivacyGuard::from_globs(&[]).unwrap();
    let policy = Policy::empty();
    let bus = Arc::new(DomainEventBus::new(64));
    let (tx, _rx) = mpsc::channel::<ToolEvent>(32);
    let pending = Arc::new(PendingApprovalsMap::new());

    let pending2 = pending.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
        pending2.resolve(
            "r3",
            ApprovalDecision::Auto {
                allowed: true,
                layer: ApprovalLayer::Layer1Declarative,
                reason: "user click".into(),
                rule_matched: None,
            },
        );
    });

    let ctx = GuardCtx {
        layer1: &l1,
        policy: &policy,
        privacy: &privacy,
        pending: &pending,
        event_tx: Some(&tx),
        domain_bus: &bus,
        cancel: CancellationToken::new(),
        request_id: "r3".into(),
        args: None,
        cwd: None,
        channel: Channel::Coding,
        non_ui_policy: NonUiPolicy::Allow,
        history_repo: None,
        repo_id: String::new(),
        mirror_learning_enabled: false,
        mirror_min_approvals: 5,
        mirror_cooldown_seconds: 86400,
        now_unix: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64,
        thread_id: None,
        turn_id: None,
    };
    let d = evaluate(ctx, "bash", "rm something").await;
    assert!(d.allowed());
}

#[tokio::test]
async fn layer3_auto_allows_after_5_prior_approvals_when_enabled() {
    use storage::repos::{CodingApprovalHistoryRepo, HistoryEntry};
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    let history = CodingApprovalHistoryRepo::new(pool.clone());
    for _ in 0..5 {
        history
            .record(HistoryEntry {
                tool: "bash".into(),
                args_hash: klynt_core::approval::layer3::args_hash_for_relevance(
                    "bash",
                    r#"{"command":"echo hi"}"#,
                ),
                repo_id: "test-repo".into(),
                decision: "allow".into(),
                decided_by: "user".into(),
                layer: "ask".into(),
            })
            .await
            .unwrap();
    }
    let perms = CodingPermissions {
        allow: vec![],
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

    let ctx = guard_ctx_with_history(
        &l1,
        &policy,
        &privacy,
        &pending,
        None,
        &bus,
        Some(Arc::new(history.clone())),
        true,
        25,
    );
    let decision = evaluate(ctx, "bash", r#"{"command":"echo hi"}"#).await;
    assert!(matches!(decision.layer(), ApprovalLayer::Layer3Mirror));
    assert!(decision.allowed(), "expected auto-allow, got {decision:?}");
}
