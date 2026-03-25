//! Integration tests for the Mirror self-reflection system.
//!
//! Verifies the end-to-end flow: routing snapshot insertion via MirrorRepo
//! → state and history queries via MirrorFacade.

use klyntbot::cognitive::mirror::{
    BrainVersion, MetaRule, MetaRuleAction, MetaRuleSource, MetaRuleStatus, MirrorFacade,
    MirrorRepo, RoutingSnapshot, SkillRouteStats,
};
use klyntbot::cognitive::repos::cognitive_migrations;
use klyntbot::storage::StoragePool;

use super::common::test_pool;

use chrono::Utc;
use std::collections::HashMap;
use uuid::Uuid;

/// Create a test pool with cognitive migrations applied (mirrors the pattern in cognitive.rs).
async fn mirror_pool() -> klyntbot::storage::StoragePool {
    let pool = test_pool().await;
    klyntbot::storage::StoragePool::run_feature_migrations(pool.inner(), &cognitive_migrations())
        .await
        .expect("cognitive migrations");
    pool
}

#[tokio::test]
async fn test_mirror_routing_accumulation_and_facade() {
    // 1. Create in-memory pool with all migrations applied.
    let pool = mirror_pool().await;

    // 2. Create MirrorRepo and MirrorFacade.
    let repo = MirrorRepo::new(pool.clone());
    let facade = MirrorFacade::new(repo.clone());

    // 3. Insert a routing snapshot (simulating what RoutingMirrorSubscriber flushes).
    let snapshot = RoutingSnapshot {
        id: Uuid::new_v4(),
        captured_at: Utc::now(),
        window_hours: 1,
        total_messages: 100,
        distribution: HashMap::from([
            (
                "general".to_string(),
                SkillRouteStats {
                    count: 60,
                    percentage: 60.0,
                    avg_confidence: 0.75,
                    top_triggers: vec![],
                },
            ),
            (
                "finance-management".to_string(),
                SkillRouteStats {
                    count: 40,
                    percentage: 40.0,
                    avg_confidence: 0.85,
                    top_triggers: vec!["budget".to_string()],
                },
            ),
        ]),
        fallback_rate: 0.60,
        avg_routing_confidence: 0.79,
        low_confidence_count: 5,
        user_feedback: None,
    };
    repo.insert_routing_snapshot(&snapshot).await.unwrap();

    // 4. Query via facade — state should reflect the inserted snapshot.
    let state = facade.get_state().await.unwrap();
    assert!(
        state.last_routing_snapshot.is_some(),
        "Expected a routing snapshot in state"
    );
    let snap = state.last_routing_snapshot.unwrap();
    assert_eq!(snap.total_messages, 100);
    assert_eq!(snap.distribution.len(), 2);
    assert!(snap.distribution.contains_key("general"));
    assert!(snap.distribution.contains_key("finance-management"));

    // 5. Routing history within a 7-day window should return the snapshot.
    let history = facade.get_routing_history(7).await.unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].total_messages, 100);

    // 6. No narratives have been generated yet — list should be empty.
    let narratives = facade.get_narratives(10).await.unwrap();
    assert!(narratives.is_empty(), "Expected no trend narratives");

    // 7. No snippets have been inserted — pending list should be empty.
    let snippets = facade.get_pending_snippets().await.unwrap();
    assert!(snippets.is_empty(), "Expected no pending snippets");
}

#[tokio::test]
async fn test_mirror_meta_rule_lifecycle() {
    // 1. Create in-memory pool with cognitive migrations applied.
    let pool = mirror_pool().await;

    let repo = MirrorRepo::new(pool.clone());
    let facade = MirrorFacade::new(repo.clone());

    // 2. Insert a pending meta-rule.
    let rule_id = Uuid::new_v4();
    let rule = MetaRule {
        id: rule_id,
        trigger_condition: "When corrected twice on finance, clarify first".to_string(),
        action: MetaRuleAction::ForceClarification,
        source: MetaRuleSource::CorrectionDerived,
        effectiveness_score: 0.5,
        status: MetaRuleStatus::Pending,
        signal_count: 0,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    repo.insert_meta_rule(&rule).await.unwrap();

    // 3. Verify it appears in state as pending.
    let state = facade.get_state().await.unwrap();
    assert_eq!(state.pending_meta_rules.len(), 1);
    assert!(state.active_meta_rules.is_empty());

    // 4. Approve the rule.
    facade.approve_meta_rule(rule_id).await.unwrap();

    // 5. Verify it has moved to active.
    let state = facade.get_state().await.unwrap();
    assert!(state.pending_meta_rules.is_empty());
    assert_eq!(state.active_meta_rules.len(), 1);

    // 6. Signal tracking — increment once and verify.
    repo.increment_meta_rule_signal(rule_id).await.unwrap();
    let (active, _) = facade.get_meta_rules().await.unwrap();
    assert_eq!(active[0].signal_count, 1);
}

#[tokio::test]
async fn test_mirror_brain_version_lifecycle() {
    let pool = test_pool().await;
    StoragePool::run_feature_migrations(pool.inner(), &cognitive_migrations())
        .await
        .unwrap();

    let repo = MirrorRepo::new(pool.clone());
    let facade = MirrorFacade::new(repo.clone());

    // 1. Insert v1 and v2
    for i in 1..=2u32 {
        let v = BrainVersion {
            version: i,
            trial_id: None,
            promoted_at: chrono::Utc::now(),
            params: serde_json::json!({"version": i}),
            reason: format!("Version {i}"),
            parent_version: if i > 1 { Some(i - 1) } else { None },
            metrics_at_promotion: serde_json::json!({}),
            reverted: false,
        };
        repo.insert_brain_version(&v).await.unwrap();
    }

    // 2. Verify in state
    let state = facade.get_state().await.unwrap();
    assert!(state.latest_brain_version.is_some());
    assert_eq!(state.latest_brain_version.unwrap().version, 2);

    // 3. Get all versions
    let versions = facade.get_brain_versions().await.unwrap();
    assert_eq!(versions.len(), 2);

    // 4. Revert to v1 (DB only)
    let new_v = facade.revert_to_version_db_only(1).await.unwrap();
    assert_eq!(new_v.version, 3);
    assert_eq!(new_v.reason, "Reverted to v1");

    // 5. Verify v2 reverted, v1 and v3 not
    let versions = facade.get_brain_versions().await.unwrap();
    assert_eq!(versions.len(), 3);
    assert!(!versions.iter().find(|v| v.version == 1).unwrap().reverted);
    assert!(versions.iter().find(|v| v.version == 2).unwrap().reverted);
    assert!(!versions.iter().find(|v| v.version == 3).unwrap().reverted);
}
