//! Phase 6 selective-delete signal.
//!
//! Find memories retrieved ≥ N times with zero citations; multiply their
//! stability by 0.5; log to `selective_delete_log`. **No row deletion ever.**

use common::{KlyntbotError, Result};
use jiff::Timestamp;
use uuid::Uuid;

/// Signal applier.
#[derive(Debug, Default)]
pub struct SelectiveDeleteSignal;

/// Repo for the audit log.
#[derive(Debug, Clone)]
pub struct SelectiveDeleteLogRepo {
    pool: storage::StoragePool,
}

impl SelectiveDeleteLogRepo {
    /// Construct.
    pub fn new(pool: storage::StoragePool) -> Self {
        Self { pool }
    }

    /// Pool ref.
    pub fn pool(&self) -> &storage::StoragePool {
        &self.pool
    }

    /// Insert one audit row.
    pub async fn insert(
        &self,
        memory_id: &str,
        memory_kind: &str,
        retrievals: u32,
        citations: u32,
        before: f32,
        after: f32,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO selective_delete_log \
             (id, memory_id, memory_kind, retrievals_observed, citations_observed, \
              stability_before, stability_after) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )
        .bind(format!("sd_{}", Uuid::new_v4().simple()))
        .bind(memory_id)
        .bind(memory_kind)
        .bind(retrievals as i64)
        .bind(citations as i64)
        .bind(before)
        .bind(after)
        .execute(self.pool.inner())
        .await
        .map_err(|e| KlyntbotError::Storage(format!("selective_delete_log insert: {e}")))?;
        Ok(())
    }
}

impl SelectiveDeleteSignal {
    /// Apply with default threshold 5.
    pub async fn apply(pool: &storage::StoragePool, log: &SelectiveDeleteLogRepo) -> Result<u32> {
        Self::apply_with_threshold(pool, log, 5).await
    }

    /// Apply with explicit threshold.
    pub async fn apply_with_threshold(
        pool: &storage::StoragePool,
        log: &SelectiveDeleteLogRepo,
        threshold: u32,
    ) -> Result<u32> {
        let candidates: Vec<(String, i64, i64)> = sqlx::query_as(
            "SELECT memory_id, COUNT(*), SUM(CASE WHEN cited_in_response THEN 1 ELSE 0 END) \
             FROM memory_utilization \
             GROUP BY memory_id \
             HAVING COUNT(*) >= ?1 AND SUM(CASE WHEN cited_in_response THEN 1 ELSE 0 END) = 0",
        )
        .bind(threshold as i64)
        .fetch_all(pool.inner())
        .await
        .map_err(|e| KlyntbotError::Storage(format!("uncited query: {e}")))?;

        let now = Timestamp::now().to_string();
        let mut applied = 0_u32;
        for (memory_id, retrievals, _) in candidates {
            // Try semantic_facts first, then episodic_memories.
            let (kind, prior_stability) =
                match fetch_stability(pool, &memory_id, "semantic_facts").await? {
                    Some(s) => ("semantic_fact", s),
                    None => match fetch_stability(pool, &memory_id, "episodic_memories").await? {
                        Some(s) => ("episodic_memory", s),
                        None => continue,
                    },
                };
            let new_stability = prior_stability * 0.5;
            update_stability(pool, &memory_id, kind, new_stability, &now).await?;
            log.insert(
                &memory_id,
                kind,
                retrievals as u32,
                0,
                prior_stability,
                new_stability,
            )
            .await?;
            applied += 1;
        }
        Ok(applied)
    }
}

async fn fetch_stability(
    pool: &storage::StoragePool,
    id: &str,
    table: &str,
) -> Result<Option<f32>> {
    let q = format!("SELECT stability FROM {table} WHERE id = ?1");
    let row: Option<(f32,)> = sqlx::query_as(&q)
        .bind(id)
        .fetch_optional(pool.inner())
        .await
        .map_err(|e| KlyntbotError::Storage(format!("stability fetch: {e}")))?;
    Ok(row.map(|(s,)| s))
}

async fn update_stability(
    pool: &storage::StoragePool,
    id: &str,
    kind: &str,
    new_stability: f32,
    _now: &str,
) -> Result<()> {
    let table = if kind == "semantic_fact" {
        "semantic_facts"
    } else {
        "episodic_memories"
    };
    let q = format!("UPDATE {table} SET stability = ?1 WHERE id = ?2");
    sqlx::query(&q)
        .bind(new_stability)
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(|e| KlyntbotError::Storage(format!("stability update: {e}")))?;
    Ok(())
}
