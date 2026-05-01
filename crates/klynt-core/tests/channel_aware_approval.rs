//! Channel-aware approval degradation tests.
//!
//! When Layer1 returns `Ask` and the channel does not support approval UI
//! (Telegram/Discord/Slack/Email), the evaluator falls back to the
//! configured `non_ui_policy`.

use bus::DomainEventBus;
use common::tool_channel::{Channel, NonUiPolicy};
use klynt_core::approval::{evaluate, ApprovalDecision, GuardCtx, Layer1, PendingApprovalsMap};
use klynt_core::privacy::PrivacyGuard;
use klynt_execpolicy::Policy;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tools_core::events::ToolEvent;

fn build_ask_layer1() -> Arc<Layer1> {
    // A Layer1 with no rules and default_if_no_match = "ask"
    let perms = config::schema::CodingPermissions {
        allow: vec![],
        deny: vec![],
        ask: vec![],
        default_if_no_match: "ask".to_string(),
        mirror_learning: false,
        mirror_cooldown_hours: 24,
        mirror_min_approvals: 5,
    };
    Arc::new(Layer1::compile(&perms).expect("Layer1 compile"))
}

fn build_ctx<'a>(
    layer1: &'a Layer1,
    policy: &'a Policy,
    privacy: &'a PrivacyGuard,
    pending: &'a Arc<PendingApprovalsMap>,
    event_tx: Option<&'a mpsc::Sender<ToolEvent>>,
    bus: &'a Arc<DomainEventBus>,
    channel: Channel,
    non_ui_policy: NonUiPolicy,
) -> GuardCtx<'a> {
    GuardCtx {
        layer1,
        policy,
        privacy,
        pending,
        event_tx,
        domain_bus: bus,
        cancel: CancellationToken::new(),
        request_id: "test-1".into(),
        args: None,
        cwd: None,
        channel,
        non_ui_policy,
        history_repo: None,
        repo_id: String::new(),
        mirror_learning_enabled: false,
        mirror_min_approvals: 5,
        mirror_cooldown_seconds: 86400,
        now_unix: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64,
    }
}

#[tokio::test]
async fn ask_in_telegram_with_allow_policy_returns_auto_allowed() {
    let layer1 = build_ask_layer1();
    let policy = Policy::empty();
    let privacy = PrivacyGuard::from_globs(&[]).unwrap();
    let pending = Arc::new(PendingApprovalsMap::default());
    let bus = Arc::new(DomainEventBus::new(8));
    let ctx = build_ctx(
        &layer1,
        &policy,
        &privacy,
        &pending,
        None,
        &bus,
        Channel::Other,
        NonUiPolicy::Allow,
    );
    let dec = evaluate(ctx, "web_fetch", "https://example.com").await;
    assert!(matches!(dec, ApprovalDecision::Auto { allowed: true, .. }));
}

#[tokio::test]
async fn ask_in_telegram_with_deny_policy_returns_auto_denied() {
    let layer1 = build_ask_layer1();
    let policy = Policy::empty();
    let privacy = PrivacyGuard::from_globs(&[]).unwrap();
    let pending = Arc::new(PendingApprovalsMap::default());
    let bus = Arc::new(DomainEventBus::new(8));
    let ctx = build_ctx(
        &layer1,
        &policy,
        &privacy,
        &pending,
        None,
        &bus,
        Channel::Other,
        NonUiPolicy::DenyWithError,
    );
    let dec = evaluate(ctx, "web_fetch", "https://example.com").await;
    assert!(matches!(dec, ApprovalDecision::Auto { allowed: false, .. }));
}

#[tokio::test]
async fn ask_in_coding_chat_does_not_degrade() {
    let layer1 = build_ask_layer1();
    let policy = Policy::empty();
    let privacy = PrivacyGuard::from_globs(&[]).unwrap();
    let pending = Arc::new(PendingApprovalsMap::default());
    let bus = Arc::new(DomainEventBus::new(8));
    // We can't easily await round-trip in a unit test (no real UI replier),
    // so we cancel immediately and assert TimedOut.
    let token = CancellationToken::new();
    token.cancel();
    let mut ctx = build_ctx(
        &layer1,
        &policy,
        &privacy,
        &pending,
        None,
        &bus,
        Channel::Coding,
        NonUiPolicy::Allow,
    );
    ctx.cancel = token;
    let dec = evaluate(ctx, "web_fetch", "https://example.com").await;
    // In coding mode, degradation is bypassed → falls into round-trip → cancelled.
    assert!(matches!(dec, ApprovalDecision::Cancelled));
}
