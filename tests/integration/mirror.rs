//! Integration tests for the Mirror self-reflection system.
//!
//! Verifies the end-to-end flow: routing snapshot insertion via MirrorRepo
//! → state and history queries via MirrorFacade.

use klyntbot::cognitive::mirror::{MirrorFacade, MirrorRepo, RoutingSnapshot, SkillRouteStats};
use klyntbot::cognitive::repos::cognitive_migrations;

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
