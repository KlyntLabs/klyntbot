//! Dead-letter queue for observations that failed LLM extraction/consolidation.
//!
//! When the LLM call fails and the pipeline falls back to heuristic handlers,
//! the original observation is persisted here for later reprocessing.

use sqlx::SqlitePool;
use tracing::warn;

use crate::types::Observation;

/// Repository for failed observations (dead-letter queue).
#[derive(Debug, Clone)]
pub struct FailedObservationRepo {
    pool: SqlitePool,
}

/// A failed observation row from the dead-letter table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct FailedObservationRow {
    pub id: String,
    pub observation_json: String,
    pub failure_reason: String,
    pub failed_stage: String,
    pub retry_count: i32,
    pub max_retries: i32,
    pub created_at: String,
    pub next_retry_at: Option<String>,
}

impl FailedObservationRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert a failed observation into the dead-letter queue.
    pub async fn insert(&self, observation: &Observation, stage: &str, reason: &str) {
        let id = uuid::Uuid::new_v4().to_string();
        let json = match serde_json::to_string(observation) {
            Ok(j) => j,
            Err(e) => {
                warn!("Failed to serialize observation for dead-letter: {e}");
                return;
            }
        };
        if let Err(e) = sqlx::query(
            "INSERT INTO failed_observations (id, observation_json, failure_reason, failed_stage) \
             VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(&id)
        .bind(&json)
        .bind(reason)
        .bind(stage)
        .execute(&self.pool)
        .await
        {
            warn!("Failed to insert dead-letter observation: {e}");
        }
    }

    /// List observations eligible for retry.
    pub async fn list_eligible(&self, limit: i64) -> Vec<FailedObservationRow> {
        sqlx::query_as::<_, FailedObservationRow>(
            "SELECT * FROM failed_observations \
             WHERE retry_count < max_retries \
             AND (next_retry_at IS NULL OR next_retry_at <= datetime('now')) \
             ORDER BY created_at ASC LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_else(|e| {
            warn!("Failed to list eligible dead-letter observations: {e}");
            Vec::new()
        })
    }

    /// Remove a successfully reprocessed observation.
    pub async fn mark_succeeded(&self, id: &str) {
        if let Err(e) = sqlx::query("DELETE FROM failed_observations WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await
        {
            warn!("Failed to mark dead-letter observation as succeeded: {e}");
        }
    }

    /// Increment retry count and set backoff delay.
    pub async fn mark_failed(&self, id: &str) {
        if let Err(e) = sqlx::query(
            "UPDATE failed_observations \
             SET retry_count = retry_count + 1, \
                 next_retry_at = datetime('now', '+' || ((retry_count + 1) * 5) || ' minutes') \
             WHERE id = ?1",
        )
        .bind(id)
        .execute(&self.pool)
        .await
        {
            warn!("Failed to mark dead-letter observation as failed: {e}");
        }
    }

    /// Count all pending observations (including those not yet eligible for retry).
    pub async fn count_pending(&self) -> i64 {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM failed_observations WHERE retry_count < max_retries",
        )
        .fetch_one(&self.pool)
        .await
        .unwrap_or((0,));
        row.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    async fn setup() -> (SqlitePool, FailedObservationRepo) {
        let pool = crate::repos::cognitive_test_pool().await;
        let repo = FailedObservationRepo::new(pool.clone());
        (pool, repo)
    }

    fn test_observation() -> Observation {
        Observation {
            domain: "productivity".into(),
            content: "User prefers morning work".into(),
            importance: 0.8,
            source_event: "ChatTurnCompleted".into(),
            timestamp: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_insert_and_list_eligible() {
        let (_pool, repo) = setup().await;
        let obs = test_observation();

        repo.insert(&obs, "extraction", "llm_error").await;

        let eligible = repo.list_eligible(10).await;
        assert_eq!(eligible.len(), 1);
        assert_eq!(eligible[0].failure_reason, "llm_error");
        assert_eq!(eligible[0].failed_stage, "extraction");
        assert_eq!(eligible[0].retry_count, 0);
    }

    #[tokio::test]
    async fn test_mark_succeeded_removes_row() {
        let (_pool, repo) = setup().await;
        let obs = test_observation();

        repo.insert(&obs, "extraction", "llm_error").await;
        let eligible = repo.list_eligible(10).await;
        assert_eq!(eligible.len(), 1);

        repo.mark_succeeded(&eligible[0].id).await;

        let remaining = repo.list_eligible(10).await;
        assert!(remaining.is_empty());
    }

    #[tokio::test]
    async fn test_mark_failed_increments_retry() {
        let (_pool, repo) = setup().await;
        let obs = test_observation();

        repo.insert(&obs, "extraction", "parse_error").await;
        let eligible = repo.list_eligible(10).await;
        let id = &eligible[0].id;

        repo.mark_failed(id).await;

        // After mark_failed, next_retry_at is set in the future, so not eligible yet
        let eligible_now = repo.list_eligible(10).await;
        assert!(eligible_now.is_empty());
    }

    #[tokio::test]
    async fn test_max_retries_excludes_from_eligible() {
        let (_pool, repo) = setup().await;
        let obs = test_observation();

        repo.insert(&obs, "extraction", "llm_error").await;
        let eligible = repo.list_eligible(10).await;
        let id = eligible[0].id.clone();

        // Exhaust retries by directly updating the retry_count
        sqlx::query("UPDATE failed_observations SET retry_count = max_retries WHERE id = ?1")
            .bind(&id)
            .execute(&repo.pool)
            .await
            .unwrap();

        let eligible = repo.list_eligible(10).await;
        assert!(eligible.is_empty());
    }

    #[tokio::test]
    async fn test_count_pending() {
        let (_pool, repo) = setup().await;

        assert_eq!(repo.count_pending().await, 0);

        let obs = test_observation();
        repo.insert(&obs, "extraction", "llm_error").await;
        repo.insert(&obs, "consolidation", "parse_error").await;

        assert_eq!(repo.count_pending().await, 2);
    }

    #[tokio::test]
    async fn test_deserialize_observation_from_row() {
        let (_pool, repo) = setup().await;
        let obs = test_observation();

        repo.insert(&obs, "extraction", "llm_error").await;
        let rows = repo.list_eligible(10).await;

        let deserialized: Observation = serde_json::from_str(&rows[0].observation_json).unwrap();
        assert_eq!(deserialized.domain, "productivity");
        assert_eq!(deserialized.content, "User prefers morning work");
    }
}
