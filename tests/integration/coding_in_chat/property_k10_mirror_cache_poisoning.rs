use klynt_core::approval::layer3::{evaluate, Layer3Config, Layer3Outcome};
use proptest::prelude::*;
use storage::repos::{CodingApprovalHistoryRepo, HistoryEntry};
use storage::StoragePool;

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, .. ProptestConfig::default() })]

    #[test]
    fn k10_single_denial_anywhere_in_history_forces_ask(
        approvals_before in 0u32..50,
        approvals_after  in 0u32..50,
    ) {
        tokio::runtime::Runtime::new().unwrap().block_on(async move {
            let pool = StoragePool::connect_in_memory().await.unwrap();
            let repo = CodingApprovalHistoryRepo::new(pool.clone());
            for _ in 0..approvals_before {
                repo.record(HistoryEntry {
                    tool: "bash".into(), args_hash: "h".into(), repo_id: "r".into(),
                    decision: "allow".into(), decided_by: "user".into(), layer: "ask".into(),
                }).await.unwrap();
            }
            repo.record(HistoryEntry {
                tool: "bash".into(), args_hash: "h".into(), repo_id: "r".into(),
                decision: "deny".into(), decided_by: "user".into(), layer: "ask".into(),
            }).await.unwrap();
            for _ in 0..approvals_after {
                repo.record(HistoryEntry {
                    tool: "bash".into(), args_hash: "h".into(), repo_id: "r".into(),
                    decision: "allow".into(), decided_by: "user".into(), layer: "ask".into(),
                }).await.unwrap();
            }
            let s = repo.summary("bash", "h", "r").await.unwrap();
            let outcome = evaluate(
                &Layer3Config { enabled: true, min_approvals: 5, cooldown_seconds: 0 },
                &s, i64::MAX,
            );
            prop_assert!(matches!(outcome, Layer3Outcome::Ask { .. }),
                "K10 violated: a denial in history must force Ask regardless of allow count, got {outcome:?}");
            Ok(())
        }).unwrap();
    }
}
