//! Memory compaction, archival, and FSRS decay.
//!
//! Daily background job that:
//! 1. Archives superseded facts older than 90 days
//! 2. Deletes episodic memories older than 90 days with low access
//! 3. Enforces size budget on active semantic facts

use tracing::{debug, info};

use crate::repos::{EpisodicMemoryRepo, SemanticFactRepo};

/// Default: archive superseded facts older than this many days.
const DEFAULT_ARCHIVE_DAYS: i64 = 90;
/// Default: delete episodic memories older than this many days with low access.
const DEFAULT_EPISODIC_ARCHIVE_DAYS: i64 = 90;
/// Minimum access count for episodic memories to survive compaction.
const EPISODIC_MIN_ACCESS_COUNT: i64 = 2;
/// Maximum active semantic facts before aggressive compaction.
const MAX_ACTIVE_FACTS: usize = 10_000;
/// Stability threshold below which facts are candidates for compaction.
const LOW_STABILITY_THRESHOLD: f64 = 0.1;

/// Result of a compaction run.
#[derive(Debug, Clone, Default)]
pub struct CompactionResult {
    pub facts_archived: u64,
    pub episodic_deleted: u64,
    pub low_stability_archived: u64,
}

/// Run the full compaction cycle.
pub async fn run_compaction(
    fact_repo: &SemanticFactRepo,
    episodic_repo: &EpisodicMemoryRepo,
) -> Result<CompactionResult, sqlx::Error> {
    run_compaction_with_params(
        fact_repo,
        episodic_repo,
        DEFAULT_ARCHIVE_DAYS,
        DEFAULT_EPISODIC_ARCHIVE_DAYS,
        EPISODIC_MIN_ACCESS_COUNT,
    )
    .await
}

/// Run compaction with configurable parameters (for testing).
pub async fn run_compaction_with_params(
    fact_repo: &SemanticFactRepo,
    episodic_repo: &EpisodicMemoryRepo,
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

    debug!(
        "Compaction complete: {} facts archived, {} episodic deleted, {} low-stability archived",
        result.facts_archived, result.episodic_deleted, result.low_stability_archived
    );

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{EpisodicMemory, SemanticFact};

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
        let result = run_compaction_with_params(&fact_repo, &episodic_repo, 0, 90, 2)
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

        let result = run_compaction_with_params(&fact_repo, &episodic_repo, 90, 90, 2)
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

        let result = run_compaction(&fact_repo, &episodic_repo).await.unwrap();
        assert_eq!(result.facts_archived, 0);
        assert_eq!(result.episodic_deleted, 0);
        assert_eq!(result.low_stability_archived, 0);
    }
}
