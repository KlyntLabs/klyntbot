use agent::events::AgentEvent;
use bus::DomainEventBus;
use config::schema::CodingPermissions;
use klynt_core::approval::{guard::evaluate, GuardCtx, Layer1, PendingApprovalsMap};
use klynt_core::privacy::PrivacyGuard;
use klynt_execpolicy::Policy;
use proptest::prelude::*;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

proptest! {
    #![proptest_config(ProptestConfig { cases: 32, .. ProptestConfig::default() })]
    #[test]
    fn k8_request_resolve_pair(n in 1usize..15) {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async move {
            let perms = CodingPermissions {
                allow: vec!["Bash(echo *)".into()], default_if_no_match: "ask".into(),
                ..Default::default()
            };
            let l1 = Layer1::compile(&perms).unwrap();
            let policy = Policy::empty();
            let privacy = PrivacyGuard::from_globs(&[]).unwrap();
            let pending = Arc::new(PendingApprovalsMap::new());
            let bus = Arc::new(DomainEventBus::new(256));
            let (tx, mut rx) = mpsc::channel(256);

            for i in 0..n {
                let ctx = GuardCtx {
                    layer1: &l1, policy: &policy, privacy: &privacy,
                    pending: &pending, event_tx: Some(&tx), domain_bus: &bus,
                    cancel: CancellationToken::new(),
                    request_id: format!("r-{i}"),
                };
                let _ = evaluate(ctx, "bash", "echo k8").await;
            }
            drop(tx);
            let mut req = 0; let mut res = 0;
            while let Some(e) = rx.recv().await {
                match e {
                    AgentEvent::ApprovalRequested { .. } => req += 1,
                    AgentEvent::ApprovalResolved { .. }  => res += 1,
                    _ => {}
                }
            }
            prop_assert_eq!(req, n);
            prop_assert_eq!(res, n);
            Ok::<(), TestCaseError>(())
        }).unwrap();
    }
}
