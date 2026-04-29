//! KCA Phase C — Retrieval Intelligence integration tests.
//!
//! Covers:
//! - Track 6: PPR boost in UnifiedMemoryService
//! - Track 7: Predictive cache hit-rate + auto-disable
//! - Track 8: Hierarchical rollup (hourly → daily → weekly)
//! - Track 13: Temporal pruning at retrieval time

use cognitive::repos::{EntityRepo, EpisodicMemoryRepo, SemanticFactRepo};
use cognitive::services::hierarchical_compressor::HierarchicalSummarizer;
use cognitive::services::hierarchical_compressor::{Tier, roll_up_daily, roll_up_hourly};
use cognitive::services::predictive_cache::{PredictiveCache, query_hash};
use cognitive::services::temporal_pruner::{
    NoopTemporalPruner, PruneFactRef, PruneInput, TemporalPrunerHandler,
};
use cognitive::types::{EpisodicMemory, SemanticFact};
use std::sync::Arc;
use std::time::Duration;

// ── Track 7: Predictive Cache ───────────────────────────────────────────────

#[tokio::test]
async fn predictive_cache_basic_hit_and_miss() {
    let cache = PredictiveCache::new(10, Duration::from_secs(60));
    let key = query_hash("test query");

    // Miss before insert
    assert!(cache.get(&key).await.is_none());

    // Insert
    let facts = vec![];
    cache.put(key.clone(), facts.clone()).await;

    // Hit after insert
    let got = cache.get(&key).await;
    assert!(got.is_some());
    assert_eq!(got.unwrap().len(), 0);

    let stats = cache.stats().await;
    assert_eq!(stats.hits, 1);
    assert_eq!(stats.misses, 1);
}

#[tokio::test]
async fn predictive_cache_ttl_expiration() {
    let cache = PredictiveCache::new(10, Duration::from_millis(10));
    let key = query_hash("ttl test");
    cache.put(key.clone(), vec![]).await;

    // Immediate hit
    assert!(cache.get(&key).await.is_some());

    // Wait for TTL
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(cache.get(&key).await.is_none());
}

#[tokio::test]
async fn predictive_cache_auto_disable_on_low_hit_rate() {
    let cache = PredictiveCache::with_auto_disable(10, Duration::from_secs(60), 0.9);

    // 100 misses → hit rate 0% < 90% → auto-disable
    for i in 0..100 {
        let _ = cache.get(&query_hash(&format!("miss{i}"))).await;
    }

    assert!(cache.is_disabled().await);
}

// ── Track 13: Temporal Pruner ───────────────────────────────────────────────

#[tokio::test]
async fn noop_temporal_pruner_keeps_all() {
    let pruner = NoopTemporalPruner;
    let input = PruneInput {
        facts: vec![
            PruneFactRef {
                fact_id: "f1".into(),
                subject: "Alice".into(),
                predicate: "works_at".into(),
                object: "OldCo".into(),
                valid_at: "2025-01-01T00:00:00Z".into(),
                valid_until: Some("2025-06-01T00:00:00Z".into()),
            },
            PruneFactRef {
                fact_id: "f2".into(),
                subject: "Alice".into(),
                predicate: "works_at".into(),
                object: "NewCo".into(),
                valid_at: "2025-06-02T00:00:00Z".into(),
                valid_until: None,
            },
        ],
        query_time: jiff::Timestamp::now().to_string(),
    };

    let output = pruner.prune(input).await.unwrap();
    assert!(output.drop.is_empty());
    assert_eq!(output.keep.len(), 2);
}

// ── Track 8: Hierarchical Rollup ────────────────────────────────────────────

struct DummySummarizer;

#[async_trait::async_trait]
impl HierarchicalSummarizer for DummySummarizer {
    async fn summarize(&self, items: &[EpisodicMemory], tier: Tier) -> common::Result<String> {
        Ok(format!("{} items at {:?}", items.len(), tier))
    }
}

#[tokio::test]
async fn hourly_rollup_groups_raw_episodes() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    let migrations = cognitive::repos::cognitive_migrations();
    storage::StoragePool::run_feature_migrations(pool.inner(), &migrations)
        .await
        .unwrap();
    let repo = EpisodicMemoryRepo::new(pool.inner().clone());

    // Insert 5 raw episodics in the current hour.
    let now = jiff::Timestamp::now();
    for i in 0..5 {
        let mut ep = raw_episode(&format!("ep{i}"), "test", &format!("event {i}"));
        ep.occurred_at = now.to_string();
        ep.recorded_at = now.to_string();
        repo.insert(&ep).await.unwrap();
    }

    let summarizer = Arc::new(DummySummarizer);
    let rolled = roll_up_hourly(&repo, summarizer).await.unwrap();
    assert_eq!(rolled, 1);

    // After rollup, raw count should be 0 unrolled.
    let unrolled = repo.list_unrolled_at_tier("raw", 100).await.unwrap();
    assert_eq!(unrolled.len(), 0);

    // Hourly tier should have 1 parent.
    let hourly = repo.list_by_tier("hourly", 100).await.unwrap();
    assert_eq!(hourly.len(), 1);
    assert_eq!(hourly[0].child_count, 5);
}

#[tokio::test]
async fn daily_rollup_groups_hourly_parents() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    let migrations = cognitive::repos::cognitive_migrations();
    storage::StoragePool::run_feature_migrations(pool.inner(), &migrations)
        .await
        .unwrap();
    let repo = EpisodicMemoryRepo::new(pool.inner().clone());

    // Seed hourly parents directly (simulating prior rollup).
    let now = jiff::Timestamp::now();
    for i in 0..3 {
        let mut ep = raw_episode(&format!("hourly{i}"), "test", &format!("hour {i}"));
        ep.tier = "hourly".into();
        ep.child_count = i as i64 + 1;
        ep.occurred_at = now.to_string();
        ep.recorded_at = now.to_string();
        repo.insert(&ep).await.unwrap();
    }

    let summarizer = Arc::new(DummySummarizer);
    let rolled = roll_up_daily(&repo, summarizer).await.unwrap();
    assert_eq!(rolled, 1);

    let daily = repo.list_by_tier("daily", 100).await.unwrap();
    assert_eq!(daily.len(), 1);
    // child_count should sum: 1 + 2 + 3 = 6
    assert_eq!(daily[0].child_count, 6);
}

// ── Track 6: PPR + Unified Retrieval integration ────────────────────────────

#[tokio::test]
async fn ppr_boost_retrieves_facts_for_related_entities() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    let migrations = cognitive::repos::cognitive_migrations();
    storage::StoragePool::run_feature_migrations(pool.inner(), &migrations)
        .await
        .unwrap();
    let entity_repo = EntityRepo::new(pool.inner().clone());
    let fact_repo = SemanticFactRepo::new(pool.inner().clone());

    // Seed entities: Alice and Bob are connected (causal).
    let alice = entity_repo
        .upsert_entity(&cognitive::repos::NewEntity {
            name: "Alice".into(),
            entity_type: "person".into(),
            description: None,
            source: "test".into(),
            source_id: None,
            metadata: None,
        })
        .await
        .unwrap();
    let bob = entity_repo
        .upsert_entity(&cognitive::repos::NewEntity {
            name: "Bob".into(),
            entity_type: "person".into(),
            description: None,
            source: "test".into(),
            source_id: None,
            metadata: None,
        })
        .await
        .unwrap();

    // Link Alice → Bob with causal edge (strongest multiplier).
    entity_repo
        .upsert_relationship_typed(&alice.id, &bob.id, "knows", "causal", 1.0, None, "test")
        .await
        .unwrap();

    // Seed a fact about Bob.
    let now = jiff::Timestamp::now().to_string();
    let bob_fact = SemanticFact {
        id: "f1".into(),
        domain: "test".into(),
        subject: "Bob".into(),
        predicate: "knows".into(),
        object: "Rust".into(),
        confidence: 0.9,
        source: "test".into(),
        valid_from: now.clone(),
        valid_until: None,
        recorded_at: now.clone(),
        superseded_at: None,
        superseded_by: None,
        stability: 1.0,
        last_accessed: None,
        access_count: 0,
        convergence_score: 0.0,
        project_id: None,
        memory_type: "fact".into(),
        scope_type: "user".into(),
        scope_id: None,
        scope_repo_id: None,
        metadata: None,
    };
    fact_repo.upsert(&bob_fact).await.unwrap();

    // Build PPR cache and retrieve with query mentioning "Alice".
    let ppr_cache = Arc::new(cognitive::services::ppr_retrieval::CachedPprGraph::new(
        entity_repo.clone(),
        Duration::from_secs(60),
    ));
    let results = cognitive::services::ppr_retrieval::retrieve_with_ppr_boost(
        &fact_repo,
        &entity_repo,
        &ppr_cache,
        "What does Alice know?",
        10,
    )
    .await
    .unwrap();

    // PPR should boost Bob's fact because Alice is connected to Bob.
    assert!(
        !results.is_empty(),
        "PPR should retrieve Bob's fact via Alice-Bob edge"
    );
    assert_eq!(results[0].fact.subject, "Bob");
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn raw_episode(id: &str, domain: &str, content: &str) -> EpisodicMemory {
    EpisodicMemory {
        id: id.into(),
        domain: domain.into(),
        content: content.into(),
        summary: None,
        importance: 0.5,
        occurred_at: jiff::Timestamp::now().to_string(),
        recorded_at: jiff::Timestamp::now().to_string(),
        stability: 1.0,
        last_accessed: None,
        access_count: 0,
        project_id: None,
        scope_type: "user".into(),
        scope_id: None,
        scope_repo_id: None,
        metadata: None,
        kind: None,
        actor_id: None,
        tier: "raw".into(),
        parent_id: None,
        child_count: 0,
        rolled_up_at: None,
    }
}
