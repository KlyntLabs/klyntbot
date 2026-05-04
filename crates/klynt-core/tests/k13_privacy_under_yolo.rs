use bus::DomainEventBus;
use common::tool_channel::{Channel, NonUiPolicy};
use config::schema::CodingPermissions;
use klynt_core::approval::{evaluate, ApprovalDecision, GuardCtx, Layer1};
use klynt_core::privacy::PrivacyGuard;
use klynt_execpolicy::Policy;
use proptest::prelude::*;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tools_core::events::ToolEvent;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn k13_privacy_inviolable_under_yolo(
        path in "/(home|tmp|var)(/[a-z]{1,8}){1,3}/\\.(ssh|aws|gnupg)/[a-z]{1,8}"
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(async {
            let exclude_globs = vec!["**/.ssh/**", "**/.aws/**", "**/.gnupg/**", "**/.env"];
            let privacy = PrivacyGuard::from_globs(&exclude_globs).unwrap();

            // YoloMode: Layer1 allows everything by default
            let perms = CodingPermissions {
                default_if_no_match: "allow".into(),
                ..Default::default()
            };
            let l1 = Layer1::compile(&perms).unwrap();
            let policy = Policy::empty();
            let bus = Arc::new(DomainEventBus::new(64));
            let (tx, _rx) = mpsc::channel::<ToolEvent>(32);
            let pending = Arc::new(klynt_core::approval::PendingApprovalsMap::new());

            let ctx = GuardCtx {
                layer1: &l1,
                policy: &policy,
                privacy: &privacy,
                pending: &pending,
                event_tx: Some(&tx),
                domain_bus: &bus,
                cancel: CancellationToken::new(),
                request_id: "k13-test".into(),
                args: None,
                cwd: None,
                channel: Channel::Coding,
                non_ui_policy: NonUiPolicy::Allow,
                history_repo: None,
                repo_id: String::new(),
                mirror_learning_enabled: false,
                mirror_min_approvals: 5,
                mirror_cooldown_seconds: 86400,
                now_unix: 0,
                thread_id: None,
                turn_id: None,
            };

            evaluate(ctx, "edit", &path).await
        });
        prop_assert!(
            matches!(result, ApprovalDecision::PrivacyDenied { .. }),
            "expected PrivacyDenied for path '{path}', got {result:?}"
        );
    }
}
