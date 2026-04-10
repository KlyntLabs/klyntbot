//! Integration tests for the Mirror self-reflection system.
//!
//! Verifies the end-to-end flow: routing snapshot insertion via MirrorRepo
//! → state and history queries via MirrorFacade.

use klyntbot::cognitive::mirror::{
    BrainVersion, MetaRule, MetaRuleAction, MetaRuleSource, MetaRuleStatus, MirrorFacade,
    MirrorRepo, PreviewRecommendation, RoutingSnapshot, SkillRouteStats, TrendDirection,
    TrialEarlySignals, TrialPreview,
};
use klyntbot::cognitive::repos::{cognitive_migrations, EpisodicMemoryRepo};
use klyntbot::storage::StoragePool;

use super::common::test_pool;

use chrono::{Duration, Utc};
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

    // 4. Revert to v1 (no bridge configured, so DB-only path is taken)
    let new_v = facade.revert_to_version(1).await.unwrap();
    assert_eq!(new_v.version, 3);
    assert_eq!(new_v.reason, "Reverted to v1");

    // 5. Verify v2 reverted, v1 and v3 not
    let versions = facade.get_brain_versions().await.unwrap();
    assert_eq!(versions.len(), 3);
    assert!(!versions.iter().find(|v| v.version == 1).unwrap().reverted);
    assert!(versions.iter().find(|v| v.version == 2).unwrap().reverted);
    assert!(!versions.iter().find(|v| v.version == 3).unwrap().reverted);
}

#[tokio::test]
async fn test_mirror_trial_preview_lifecycle() {
    let pool = mirror_pool().await;
    let repo = MirrorRepo::new(pool.clone());
    let facade = MirrorFacade::new(repo.clone());

    // Insert a trial preview
    let preview = TrialPreview {
        id: Uuid::new_v4(),
        trial_id: "trial-test-001".to_string(),
        started_at: Utc::now() - chrono::Duration::hours(4),
        preview_at: Utc::now(),
        messages_scored: 25,
        early_signals: TrialEarlySignals {
            correction_rate_delta: -0.15,
            confidence_trend: TrendDirection::Falling,
            dominant_skill_shift: None,
            messages_scored: 0,
        },
        recommendation: PreviewRecommendation::Kill,
        narrative: "Correction rate worsened".to_string(),
    };
    repo.insert_trial_preview(&preview).await.unwrap();

    // Verify in state
    let state = facade.get_state().await.unwrap();
    assert_eq!(state.recent_trial_previews.len(), 1);
    assert_eq!(
        state.recent_trial_previews[0].recommendation,
        PreviewRecommendation::Kill
    );
    assert_eq!(state.recent_trial_previews[0].trial_id, "trial-test-001");
}

#[tokio::test]
async fn test_mirror_approve_meta_rule_writes_episodic_memory() {
    let pool = mirror_pool().await;
    let repo = MirrorRepo::new(pool.clone());
    let episodic = EpisodicMemoryRepo::new(pool.inner().clone());
    let facade = MirrorFacade::new(repo.clone()).with_episodic_repo(episodic.clone());

    // Insert a pending meta-rule.
    let rule = MetaRule {
        id: Uuid::new_v4(),
        trigger_condition: "When corrected on finance, clarify first".to_string(),
        action: MetaRuleAction::ForceClarification,
        source: MetaRuleSource::CorrectionDerived,
        effectiveness_score: 0.5,
        status: MetaRuleStatus::Pending,
        signal_count: 0,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    repo.insert_meta_rule(&rule).await.unwrap();

    // Approve it — this fires a spawned task to write episodic memory.
    facade.approve_meta_rule(rule.id).await.unwrap();

    // Give the spawned task time to complete.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Verify episodic memory was written with domain "mirror".
    let memories = episodic.list_by_domain("mirror", 10).await.unwrap();
    assert!(
        memories
            .iter()
            .any(|m| m.content.contains("Approved meta-rule")),
        "Expected episodic memory for meta-rule approval, got: {memories:?}"
    );
}

#[tokio::test]
async fn test_mirror_kill_trial_writes_episodic_memory() {
    let pool = mirror_pool().await;
    let repo = MirrorRepo::new(pool.clone());
    let episodic = EpisodicMemoryRepo::new(pool.inner().clone());
    let facade = MirrorFacade::new(repo.clone()).with_episodic_repo(episodic.clone());

    // Kill a trial — this fires a spawned task to write episodic memory.
    facade.kill_trial("trial-abc-123").await.unwrap();

    // Give the spawned task time to complete.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Verify episodic memory was written.
    let memories = episodic.list_by_domain("mirror", 10).await.unwrap();
    assert!(
        memories
            .iter()
            .any(|m| m.content.contains("Killed experiment")),
        "Expected episodic memory for trial kill, got: {memories:?}"
    );
}

#[tokio::test]
async fn test_mirror_create_meta_rule_from_text() {
    let pool = mirror_pool().await;
    let repo = MirrorRepo::new(pool.clone());
    let facade = MirrorFacade::new(repo.clone());

    let rule = facade
        .create_meta_rule_from_text("When user asks about budget, use finance skill".to_string())
        .await
        .unwrap();

    assert_eq!(rule.source, MetaRuleSource::UserCreated);
    assert_eq!(rule.status, MetaRuleStatus::Pending);
    assert!((rule.effectiveness_score - 0.5).abs() < f64::EPSILON);
    assert_eq!(
        rule.trigger_condition,
        "When user asks about budget, use finance skill"
    );

    // Verify it was persisted and appears in state as pending.
    let state = facade.get_state().await.unwrap();
    assert_eq!(state.pending_meta_rules.len(), 1);
    assert_eq!(state.pending_meta_rules[0].id, rule.id);
}

#[tokio::test]
async fn test_mirror_trial_preview_cleanup() {
    let pool = mirror_pool().await;
    let repo = MirrorRepo::new(pool.clone());

    // Insert a recent preview (should survive cleanup).
    let recent = TrialPreview {
        id: Uuid::new_v4(),
        trial_id: "trial-recent".to_string(),
        started_at: Utc::now() - Duration::hours(4),
        preview_at: Utc::now(),
        messages_scored: 10,
        early_signals: TrialEarlySignals {
            correction_rate_delta: 0.0,
            confidence_trend: TrendDirection::Stable,
            dominant_skill_shift: None,
            messages_scored: 0,
        },
        recommendation: PreviewRecommendation::Continue,
        narrative: "Looking good".to_string(),
    };
    repo.insert_trial_preview(&recent).await.unwrap();

    // Insert a preview with old preview_at (100 days ago) via raw SQL.
    let old_id = Uuid::new_v4();
    let old_preview_at = (Utc::now() - Duration::days(100)).to_rfc3339();
    let old_started_at = (Utc::now() - Duration::days(101)).to_rfc3339();
    let early_signals_json = serde_json::json!({
        "correctionRateDelta": -0.1,
        "confidenceTrend": "Falling",
        "dominantSkillShift": null
    })
    .to_string();
    sqlx::query(
        r#"
        INSERT INTO mirror_trial_previews
            (id, trial_id, started_at, preview_at, messages_scored,
             early_signals_json, recommendation, narrative)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
    )
    .bind(old_id.to_string())
    .bind("trial-old")
    .bind(&old_started_at)
    .bind(&old_preview_at)
    .bind(5_i64)
    .bind(&early_signals_json)
    .bind("Kill")
    .bind("Stale preview")
    .execute(pool.inner())
    .await
    .unwrap();

    // Verify both exist.
    let all = repo.get_recent_trial_previews().await.unwrap();
    assert_eq!(all.len(), 2);

    // Cleanup previews older than 30 days.
    let deleted = repo.cleanup_old_trial_previews(30).await.unwrap();
    assert_eq!(deleted, 1);

    // Only the recent one should remain.
    let remaining = repo.get_recent_trial_previews().await.unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].trial_id, "trial-recent");
}
