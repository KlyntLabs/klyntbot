//! Memory compaction, archival, and FSRS decay.
//!
//! Daily background job that:
//! 1. Archives superseded facts older than 90 days
//! 2. Deletes episodic memories older than 90 days with low access
//! 3. Enforces size budget on active semantic facts

use tracing::{debug, info, warn};

use crate::repos::{
    AccumulatedObservationRepo, CoActivationRepo, EnhancementTraceRepo, EpisodicMemoryRepo,
    FailedObservationRepo, ProceduralRuleRepo, SemanticFactRepo,
};

/// Default: archive superseded facts older than this many days.
const DEFAULT_ARCHIVE_DAYS: i64 = 90;
/// Default: delete episodic memories older than this many days with low access.
const DEFAULT_EPISODIC_ARCHIVE_DAYS: i64 = 90;
/// Minimum access count for episodic memories to survive compaction.
const EPISODIC_MIN_ACCESS_COUNT: i64 = 2;
/// Default: deactivate procedural rules older than this many days with low signal count.
const DEFAULT_RULE_STALE_DAYS: i64 = 90;
/// Minimum signal count for a rule to survive stale compaction.
const RULE_MIN_SIGNALS: i64 = 2;
/// Maximum active semantic facts before aggressive compaction.
const MAX_ACTIVE_FACTS: usize = 10_000;
/// Stability threshold below which facts are candidates for compaction.
const LOW_STABILITY_THRESHOLD: f64 = 0.1;
/// Delete accumulated observations older than this many days.
const ACCUMULATED_OBS_MAX_DAYS: i64 = 7;
/// Delete failed observations older than this many days.
const FAILED_OBS_MAX_DAYS: i64 = 30;
/// Delete session memory entries older than this many days.
const SESSION_MEMORY_MAX_DAYS: i64 = 90;
/// Weekly co-activation decay multiplier.
const CO_ACTIVATION_DECAY_FACTOR: f64 = 0.95;
/// Minimum co-activation strength to survive pruning.
const CO_ACTIVATION_MIN_STRENGTH: f64 = 0.1;
/// Delete co-activation pairs not fired in this many days, regardless of strength.
const CO_ACTIVATION_MAX_AGE_DAYS: i64 = 90;
/// Delete enhancement trace records older than this many days.
const ENHANCEMENT_TRACE_MAX_DAYS: i64 = 7;

/// Result of a compaction run.
#[derive(Debug, Clone, Default)]
pub struct CompactionResult {
    pub facts_archived: u64,
    pub episodic_deleted: u64,
    pub low_stability_archived: u64,
    pub rules_deactivated: u64,
    pub accumulated_obs_deleted: u64,
    pub failed_obs_deleted: u64,
    pub session_memory_deleted: u64,
    pub co_activation_pruned: u64,
    pub co_activation_expired: u64,
    pub enhancement_traces_deleted: u64,
}

/// Run the full compaction cycle.
#[allow(clippy::too_many_arguments)]
pub async fn run_compaction(
    fact_repo: &SemanticFactRepo,
    episodic_repo: &EpisodicMemoryRepo,
    rule_repo: Option<&ProceduralRuleRepo>,
    accum_repo: Option<&AccumulatedObservationRepo>,
    failed_repo: Option<&FailedObservationRepo>,
    session_mem_repo: Option<&storage::SessionMemoryRepo>,
    co_activation_repo: Option<&CoActivationRepo>,
    enhancement_trace_repo: Option<&EnhancementTraceRepo>,
) -> Result<CompactionResult, sqlx::Error> {
    let mut result = run_compaction_with_params(
        fact_repo,
        episodic_repo,
        rule_repo,
        DEFAULT_ARCHIVE_DAYS,
        DEFAULT_EPISODIC_ARCHIVE_DAYS,
        EPISODIC_MIN_ACCESS_COUNT,
    )
    .await?;

    // 5. Clean old accumulated observations
    if let Some(ar) = accum_repo {
        let deleted = ar.delete_older_than(ACCUMULATED_OBS_MAX_DAYS).await?;
        result.accumulated_obs_deleted = deleted;
        if deleted > 0 {
            info!("Compaction: deleted {deleted} old accumulated observations");
        }
    }

    // 6. Clean old failed observations
    if let Some(fr) = failed_repo {
        let deleted = fr.delete_older_than(FAILED_OBS_MAX_DAYS).await?;
        result.failed_obs_deleted = deleted;
        if deleted > 0 {
            info!("Compaction: deleted {deleted} old failed observations");
        }
    }

    // 7. Clean old session memory entries
    if let Some(sm) = session_mem_repo {
        match sm.delete_older_than(SESSION_MEMORY_MAX_DAYS).await {
            Ok(deleted) => {
                result.session_memory_deleted = deleted;
                if deleted > 0 {
                    info!("Compaction: deleted {deleted} old session memory entries");
                }
            }
            Err(e) => {
                warn!("Compaction: failed to clean session memory: {e}");
            }
        }
    }

    // 8. Co-activation maintenance
    if let Some(ca) = co_activation_repo {
        // 8a. Expire stale pairs (runs every cycle)
        let expired = ca.expire_stale(CO_ACTIVATION_MAX_AGE_DAYS).await?;
        result.co_activation_expired = expired;
        if expired > 0 {
            info!("Compaction: expired {expired} stale co-activation pairs (>{CO_ACTIVATION_MAX_AGE_DAYS}d)");
        }

        // 8b. Weekly strength decay (Sundays only)
        if jiff::Timestamp::now().strftime("%A").to_string() == "Sunday" {
            let pruned = ca
                .decay_all(CO_ACTIVATION_DECAY_FACTOR, CO_ACTIVATION_MIN_STRENGTH)
                .await?;
            result.co_activation_pruned = pruned;
            if pruned > 0 {
                info!("Compaction: pruned {pruned} weak co-activation pairs");
            }
        }
    }

    // 9. Clean old enhancement trace records
    if let Some(etr) = enhancement_trace_repo {
        match etr.delete_older_than(ENHANCEMENT_TRACE_MAX_DAYS).await {
            Ok(deleted) => {
                result.enhancement_traces_deleted = deleted;
                if deleted > 0 {
                    info!("Compaction: deleted {deleted} old enhancement trace records");
                }
            }
            Err(e) => {
                warn!("Compaction: failed to clean enhancement traces: {e}");
            }
        }
    }

    debug!(
        "Compaction complete (extended): {} accum_obs deleted, {} failed_obs deleted, {} session_mem deleted, {} co-activation pruned, {} enhancement_traces deleted",
        result.accumulated_obs_deleted, result.failed_obs_deleted, result.session_memory_deleted, result.co_activation_pruned, result.enhancement_traces_deleted
    );

    Ok(result)
}

/// Run compaction with configurable parameters (for testing).
pub async fn run_compaction_with_params(
    fact_repo: &SemanticFactRepo,
    episodic_repo: &EpisodicMemoryRepo,
    rule_repo: Option<&ProceduralRuleRepo>,
    archive_days: i64,
    episodic_archive_days: i64,
    min_access_count: i64,
) -> Result<CompactionResult, sqlx::Error> {
    let mut result = CompactionResult::default();

    // 1. Archive superseded facts
    let archived = fact_repo.archive_superseded(archive_days).await?;
    result.facts_archived = archived;
    if archived > 0 {
        info!("Compaction: archived {archived} superseded facts");
    }

    // 2. Delete old episodic memories with low access
    let deleted = episodic_repo
        .delete_old(episodic_archive_days, min_access_count)
        .await?;
    result.episodic_deleted = deleted;
    if deleted > 0 {
        info!("Compaction: deleted {deleted} old episodic memories");
    }

    // 3. Enforce size budget — archive low-stability facts if over limit
    let low_stability = fact_repo
        .list_low_stability(LOW_STABILITY_THRESHOLD)
        .await?;
    if low_stability.len() > MAX_ACTIVE_FACTS {
        // Archive the lowest-stability facts to get back under budget
        let excess = low_stability.len() - MAX_ACTIVE_FACTS;
        let to_archive: Vec<_> = low_stability.iter().take(excess).collect();
        for fact in &to_archive {
            // Mark as expired
            fact_repo.supersede(&fact.id, "compaction").await?;
        }
        let archived_excess = fact_repo.archive_superseded(0).await?;
        result.low_stability_archived = archived_excess;
        info!("Compaction: aggressively archived {archived_excess} low-stability facts");
    }

    // 4. Deactivate stale procedural rules
    if let Some(rr) = rule_repo {
        let deactivated = rr
            .deactivate_stale(DEFAULT_RULE_STALE_DAYS, RULE_MIN_SIGNALS)
            .await?;
        result.rules_deactivated = deactivated;
        if deactivated > 0 {
            info!("Compaction: deactivated {deactivated} stale procedural rules");
        }
    }

    debug!(
        "Compaction complete: {} facts archived, {} episodic deleted, {} low-stability archived, {} rules deactivated",
        result.facts_archived, result.episodic_deleted, result.low_stability_archived, result.rules_deactivated
    );

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{EpisodicMemory, SemanticFact, DEFAULT_MEMORY_TYPE};

    async fn setup() -> sqlx::SqlitePool {
        crate::repos::cognitive_test_pool().await
    }

    fn test_fact(id: &str) -> SemanticFact {
        SemanticFact {
            id: id.into(),
            domain: "test".into(),
            subject: "user".into(),
            predicate: "pref".into(),
            object: "val".into(),
            confidence: 0.8,
            source: "observed".into(),
            valid_from: "2026-01-01".into(),
            valid_until: None,
            recorded_at: "2026-01-01".into(),
            superseded_at: None,
            superseded_by: None,
            stability: 0.05,
            last_accessed: None,
            access_count: 0,
            convergence_score: 0.0,
            project_id: None,
            memory_type: DEFAULT_MEMORY_TYPE.to_string(),
            scope_type: "system".to_string(),
            scope_id: None,
            scope_repo_id: None,
            metadata: None,
            speaker: None,
        }
    }

    fn test_episodic(id: &str, occurred_at: &str, access_count: i64) -> EpisodicMemory {
        EpisodicMemory {
            id: id.into(),
            domain: "test".into(),
            content: "test memory".into(),
            summary: None,
            importance: 0.5,
            occurred_at: occurred_at.into(),
            recorded_at: occurred_at.into(),
            stability: 1.0,
            last_accessed: None,
            access_count,
            project_id: None,
            scope_type: "system".to_string(),
            scope_id: None,
            scope_repo_id: None,
            metadata: None,
            kind: None,
            actor_id: None,
            tier: "raw".to_string(),
            parent_id: None,
            child_count: 0,
            rolled_up_at: None,
        }
    }

    #[tokio::test]
    async fn test_compaction_archives_superseded() {
        let pool = setup().await;
        let fact_repo = SemanticFactRepo::new(pool.clone());
        let episodic_repo = EpisodicMemoryRepo::new(pool);

        let f1 = test_fact("f1");
        fact_repo.upsert(&f1).await.unwrap();
        fact_repo.supersede("f1", "f2").await.unwrap();

        // Archive with 0-day threshold (archive everything superseded)
        let result = run_compaction_with_params(&fact_repo, &episodic_repo, None, 0, 90, 2)
            .await
            .unwrap();
        assert_eq!(result.facts_archived, 1);

        // Fact should no longer be in active table
        let active = fact_repo.list_active("test").await.unwrap();
        assert!(active.is_empty());
    }

    #[tokio::test]
    async fn test_compaction_deletes_old_episodic() {
        let pool = setup().await;
        let fact_repo = SemanticFactRepo::new(pool.clone());
        let episodic_repo = EpisodicMemoryRepo::new(pool);

        // Old memory with low access
        let old = test_episodic("e1", "2025-01-01", 0);
        episodic_repo.insert(&old).await.unwrap();

        // Recent memory
        let recent = test_episodic("e2", "2026-03-05", 0);
        episodic_repo.insert(&recent).await.unwrap();

        let result = run_compaction_with_params(&fact_repo, &episodic_repo, None, 90, 90, 2)
            .await
            .unwrap();
        assert_eq!(result.episodic_deleted, 1);

        // Recent memory should still exist
        let remaining = episodic_repo.list_by_domain("test", 10).await.unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, "e2");
    }

    #[tokio::test]
    async fn test_compaction_no_op_when_empty() {
        let pool = setup().await;
        let fact_repo = SemanticFactRepo::new(pool.clone());
        let episodic_repo = EpisodicMemoryRepo::new(pool);

        let result = run_compaction(
            &fact_repo,
            &episodic_repo,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
        assert_eq!(result.facts_archived, 0);
        assert_eq!(result.episodic_deleted, 0);
        assert_eq!(result.low_stability_archived, 0);
    }

    #[tokio::test]
    async fn test_compaction_deactivates_stale_rules() {
        let pool = setup().await;
        let fact_repo = SemanticFactRepo::new(pool.clone());
        let episodic_repo = EpisodicMemoryRepo::new(pool.clone());
        let rule_repo = crate::repos::ProceduralRuleRepo::new(pool);

        let r = crate::types::ProceduralRule {
            id: "old-rule".into(),
            domain: "productivity".into(),
            rule_text: "Outdated pattern".into(),
            confidence: 0.5,
            source: "reflected".into(),
            signal_count: 0,
            created_at: "2025-01-01".into(),
            updated_at: "2025-01-01".into(),
            active: true,
            project_id: None,
            scope_type: "system".into(),
            scope_id: None,
            effectiveness_score: 0.5,
            stability: 1.0,
            scope_repo_id: None,
            last_applied: None,
            application_count: 0,
            metadata: None,
        };
        rule_repo.upsert(&r).await.unwrap();

        let result = run_compaction(
            &fact_repo,
            &episodic_repo,
            Some(&rule_repo),
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
        assert_eq!(result.rules_deactivated, 1);

        let active = rule_repo.list_active("productivity").await.unwrap();
        assert!(active.is_empty());
    }

    #[tokio::test]
    async fn test_compaction_cleans_accumulated_observations() {
        let pool = setup().await;
        let fact_repo = SemanticFactRepo::new(pool.clone());
        let episodic_repo = EpisodicMemoryRepo::new(pool.clone());
        let accum_repo = AccumulatedObservationRepo::new(pool.clone());

        // Insert old observation directly with an old timestamp
        sqlx::query(
            "INSERT INTO accumulated_observations \
             (id, event_type_key, domain, content, importance, source_event, observed_at, day_key) \
             VALUES ('old-1', 'OldEvent', 'test', 'old content', 0.5, 'ev', '2020-01-01T00:00:00Z', '2020-01-01')",
        )
        .execute(&pool)
        .await
        .unwrap();

        let result = run_compaction(
            &fact_repo,
            &episodic_repo,
            None,
            Some(&accum_repo),
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
        assert_eq!(result.accumulated_obs_deleted, 1);
    }

    #[tokio::test]
    async fn test_compaction_cleans_failed_observations() {
        let pool = setup().await;
        let fact_repo = SemanticFactRepo::new(pool.clone());
        let episodic_repo = EpisodicMemoryRepo::new(pool.clone());
        let failed_repo = FailedObservationRepo::new(pool.clone());

        // Insert an old failed observation directly
        sqlx::query(
            "INSERT INTO failed_observations \
             (id, observation_json, failure_reason, failed_stage, created_at) \
             VALUES ('old-f1', '{}', 'test', 'extraction', '2020-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();

        let result = run_compaction(
            &fact_repo,
            &episodic_repo,
            None,
            None,
            Some(&failed_repo),
            None,
            None,
            None,
        )
        .await
        .unwrap();
        assert_eq!(result.failed_obs_deleted, 1);
    }
}
