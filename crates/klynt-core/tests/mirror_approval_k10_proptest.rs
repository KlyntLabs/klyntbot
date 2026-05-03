//! K10 proptest: Mirror cache poisoning — a single Deny forces always-ask.
//!
//! Invariant: Regardless of how many approvals precede or follow a denial,
//! the `evaluate` function must always return `Layer3Outcome::Ask`.

use proptest::prelude::*;
use storage::repos::ApprovalHistorySummary;

use klynt_core::approval::layer3::{evaluate, Layer3Config, Layer3Outcome};

proptest! {
    #[test]
    fn k10_single_denial_forces_always_ask(
        enabled in any::<bool>(),
        min_approvals in 0u32..100,
        cooldown in 0i64..100_000,
        approvals in 0u32..1000,
        denials in 1u32..100,
        last_decided in any::<i64>(),
        now_unix in any::<i64>(),
    ) {
        let cfg = Layer3Config {
            enabled,
            min_approvals,
            cooldown_seconds: cooldown,
        };
        let summary = ApprovalHistorySummary {
            approval_count: approvals,
            denial_count: denials,
            last_decided_at: Some(last_decided),
        };
        let outcome = evaluate(&cfg, &summary, now_unix);
        // When enabled and there's at least one denial, must be Ask
        if cfg.enabled {
            prop_assert!(
                matches!(outcome, Layer3Outcome::Ask { .. }),
                "expected Ask when denial_count >= 1, got {:?}",
                outcome
            );
        }
    }

    #[test]
    fn k10_no_denials_can_auto_allow(
        min_approvals in 1u32..10,
        cooldown in 1i64..1000,
        now_unix in 100_000i64..200_000,
    ) {
        let cfg = Layer3Config {
            enabled: true,
            min_approvals,
            cooldown_seconds: cooldown,
        };
        let summary = ApprovalHistorySummary {
            approval_count: min_approvals + 10,
            denial_count: 0,
            last_decided_at: Some(0),
        };
        let outcome = evaluate(&cfg, &summary, now_unix);
        prop_assert!(
            matches!(outcome, Layer3Outcome::AutoAllow { .. }),
            "expected AutoAllow with enough approvals and no denials, got {:?}",
            outcome
        );
    }
}
