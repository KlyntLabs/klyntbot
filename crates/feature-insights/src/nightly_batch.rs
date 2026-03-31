//! Nightly cross-domain insight batch service.
//!
//! Reads today's surfaced cross-domain dots and stores polished LLM-generated
//! insight sentences for the next morning briefing. LLM call and cron wiring
//! are deferred to integration with the scheduling system.

use storage::{StorageError, StoragePool};

/// A row from the `cross_domain_insights` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CrossDomainInsightRow {
    pub id: i64,
    pub date: String,
    pub insight_text: String,
    pub dot_refs: String,
    pub surfaced: bool,
    pub created_at: String,
}

/// Service for storing and retrieving nightly cross-domain insights.
#[derive(Debug, Clone)]
pub struct NightlyBatchService {
    pool: StoragePool,
}

impl NightlyBatchService {
    pub fn new(pool: StoragePool) -> Self {
        Self { pool }
    }

    /// Fetch today's surfaced cross-domain dots from `brain_signal_feedback`.
    ///
    /// Returns `(entity_pair, signal_type)` pairs for dots surfaced today.
    pub async fn get_todays_dots(&self) -> Result<Vec<(String, String)>, StorageError> {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT entity_pair, signal_type FROM brain_signal_feedback
             WHERE signal_type = 'cross_domain_dot'
               AND action = 'surfaced'
               AND date(timestamp) = date('now')",
        )
        .fetch_all(self.pool.inner())
        .await?;
        Ok(rows)
    }

    /// Store a polished insight for tomorrow's morning briefing.
    pub async fn store_insight(
        &self,
        date: &str,
        insight_text: &str,
        dot_refs: &str,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO cross_domain_insights (date, insight_text, dot_refs) VALUES (?1, ?2, ?3)",
        )
        .bind(date)
        .bind(insight_text)
        .bind(dot_refs)
        .execute(self.pool.inner())
        .await?;
        Ok(())
    }

    /// Get unsurfaced insights (for morning briefing check), newest first, capped at 3.
    pub async fn get_unsurfaced_insights(
        &self,
    ) -> Result<Vec<CrossDomainInsightRow>, StorageError> {
        let rows = sqlx::query_as::<_, CrossDomainInsightRow>(
            "SELECT * FROM cross_domain_insights
             WHERE surfaced = 0
             ORDER BY created_at DESC
             LIMIT 3",
        )
        .fetch_all(self.pool.inner())
        .await?;
        Ok(rows)
    }

    /// Mark an insight as surfaced so it is not shown again.
    pub async fn mark_surfaced(&self, id: i64) -> Result<(), StorageError> {
        sqlx::query("UPDATE cross_domain_insights SET surfaced = 1 WHERE id = ?1")
            .bind(id)
            .execute(self.pool.inner())
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_store_and_get_insight() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let svc = NightlyBatchService::new(pool);

        svc.store_insight(
            "2026-03-30",
            "Finance stress clusters around month-end sprints.",
            "finance:tasks",
        )
        .await
        .unwrap();

        let unsurfaced = svc.get_unsurfaced_insights().await.unwrap();
        assert_eq!(unsurfaced.len(), 1);
        assert_eq!(unsurfaced[0].date, "2026-03-30");
        assert_eq!(
            unsurfaced[0].insight_text,
            "Finance stress clusters around month-end sprints."
        );
        assert_eq!(unsurfaced[0].dot_refs, "finance:tasks");
        assert!(!unsurfaced[0].surfaced);
    }

    #[tokio::test]
    async fn test_mark_surfaced() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let svc = NightlyBatchService::new(pool);

        svc.store_insight(
            "2026-03-30",
            "Learning sessions spike before deadlines.",
            "learning:tasks",
        )
        .await
        .unwrap();

        let unsurfaced = svc.get_unsurfaced_insights().await.unwrap();
        assert_eq!(unsurfaced.len(), 1);
        let id = unsurfaced[0].id;

        svc.mark_surfaced(id).await.unwrap();

        let after = svc.get_unsurfaced_insights().await.unwrap();
        assert!(
            after.is_empty(),
            "surfaced insight should not appear in unsurfaced list"
        );
    }
}
