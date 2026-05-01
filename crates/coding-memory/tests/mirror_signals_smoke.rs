use coding_memory::mirror::coding_signals::{
    ApprovalHistorySignal, RecallCoverageSignal, SkillEffectivenessSignal,
};
use cognitive::mirror::MirrorRepo;

async fn test_repo() -> MirrorRepo {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    storage::StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    MirrorRepo::new(pool)
}

#[tokio::test]
async fn approval_history_emits_pattern_after_threshold() {
    let repo = test_repo().await;
    let signal = ApprovalHistorySignal::new(repo);
    for _ in 0..6 {
        signal
            .observe_approval_decision("bash", "allow", "layer1")
            .await
            .unwrap();
    }
    let alerts = signal.drain().await.unwrap();
    assert!(
        alerts.iter().any(|a| matches!(a, cognitive::mirror::MirrorAlert::Coding { kind, .. } if kind == "ApprovalPatternDetected")),
        "expected pattern detection after 6 consecutive allows"
    );
}

#[tokio::test]
async fn recall_coverage_emits_low_after_3_low_turns() {
    let repo = test_repo().await;
    let signal = RecallCoverageSignal::new(repo);
    for _ in 0..3 {
        signal.observe_recall_injected(0.15, false).await.unwrap();
    }
    let alerts = signal.drain().await.unwrap();
    assert!(
        alerts.iter().any(|a| matches!(a, cognitive::mirror::MirrorAlert::Coding { kind, .. } if kind == "RecallCoverageLow")),
        "expected low coverage signal after 3 turns < 0.3"
    );
}

#[tokio::test]
async fn skill_effectiveness_emits_underperforming_after_3_failed_tool_calls() {
    let repo = test_repo().await;
    let signal = SkillEffectivenessSignal::new(repo);
    signal.observe_skill_activated("flaky-skill").await.unwrap();
    signal
        .observe_tool_result("flaky-skill", false)
        .await
        .unwrap();
    signal
        .observe_tool_result("flaky-skill", false)
        .await
        .unwrap();
    signal
        .observe_tool_result("flaky-skill", false)
        .await
        .unwrap();
    let alerts = signal.drain().await.unwrap();
    assert!(
        alerts.iter().any(|a| matches!(a, cognitive::mirror::MirrorAlert::Coding { kind, .. } if kind == "SkillUnderperforming")),
        "expected underperforming signal after 3 failed calls without success"
    );
}
