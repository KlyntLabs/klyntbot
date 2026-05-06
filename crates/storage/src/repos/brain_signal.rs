//! Brain signal feedback repository — dedup, dismissal tracking, adaptive dampening.

use sqlx::SqlitePool;

use crate::error::StorageError;

/// A row from the `brain_signal_feedback` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BrainSignalFeedbackRow {
    pub id: i64,
    pub signal_type: String,
    pub entity_pair: String,
    pub action: String,
    pub timestamp: String,
}

/// Repository for tracking which brain signals have been surfaced or dismissed.
#[derive(Debug, Clone)]
pub struct BrainSignalFeedbackRepo {
    pool: SqlitePool,
}

impl BrainSignalFeedbackRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Record a brain signal feedback event (surfaced, dismissed, accepted, etc.).
    pub async fn record(
        &self,
        signal_type: &str,
        entity_pair: &str,
        action: &str,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO brain_signal_feedback (signal_type, entity_pair, action) VALUES (?1, ?2, ?3)",
        )
        .bind(signal_type)
        .bind(entity_pair)
        .bind(action)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Returns `true` if this `entity_pair` had `action` within the last `hours` hours.
    async fn was_action_recently(
        &self,
        entity_pair: &str,
        action: &str,
        hours: i64,
    ) -> Result<bool, StorageError> {
        let cutoff = jiff::Timestamp::now().as_millisecond() - hours * 3_600_000;
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM brain_signal_feedback
             WHERE entity_pair = ?1
               AND action = ?2
               AND timestamp > ?3",
        )
        .bind(entity_pair)
        .bind(action)
        .bind(cutoff)
        .fetch_one(&self.pool)
        .await?;
        Ok(count.0 > 0)
    }

    /// Returns `true` if this `entity_pair` was surfaced within the last `hours` hours.
    pub async fn was_surfaced_recently(
        &self,
        entity_pair: &str,
        hours: i64,
    ) -> Result<bool, StorageError> {
        self.was_action_recently(entity_pair, "surfaced", hours)
            .await
    }

    /// Returns `true` if this `entity_pair` was dismissed within the last `days` days.
    pub async fn was_dismissed_recently(
        &self,
        entity_pair: &str,
        days: i64,
    ) -> Result<bool, StorageError> {
        self.was_action_recently(entity_pair, "dismissed", days * 24)
            .await
    }

    /// Count the number of dismissals recorded in the last `hours` hours (across all entity pairs).
    pub async fn dismissal_count_since(&self, hours: i64) -> Result<i64, StorageError> {
        let cutoff = jiff::Timestamp::now().as_millisecond() - hours * 3_600_000;
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM brain_signal_feedback
             WHERE action = 'dismissed'
               AND timestamp > ?1",
        )
        .bind(cutoff)
        .fetch_one(&self.pool)
        .await?;
        Ok(count.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StoragePool;

    #[tokio::test]
    async fn test_record_and_query_feedback() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = BrainSignalFeedbackRepo::new(pool.inner().clone());

        repo.record("connection", "alice:bob", "surfaced")
            .await
            .unwrap();

        // Same pair — should be true within 24h
        let found = repo.was_surfaced_recently("alice:bob", 24).await.unwrap();
        assert!(found, "should find recently surfaced pair");

        // Different pair — should be false
        let not_found = repo
            .was_surfaced_recently("charlie:dave", 24)
            .await
            .unwrap();
        assert!(!not_found, "different pair should not be found");
    }

    #[tokio::test]
    async fn test_dismissal_count() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = BrainSignalFeedbackRepo::new(pool.inner().clone());

        repo.record("connection", "alice:bob", "dismissed")
            .await
            .unwrap();
        repo.record("connection", "bob:charlie", "dismissed")
            .await
            .unwrap();
        repo.record("connection", "alice:charlie", "accepted")
            .await
            .unwrap();

        let count = repo.dismissal_count_since(1).await.unwrap();
        assert_eq!(count, 2, "should count only dismissals");
    }

    #[tokio::test]
    async fn test_dismissed_recently() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = BrainSignalFeedbackRepo::new(pool.inner().clone());

        repo.record("connection", "alice:bob", "dismissed")
            .await
            .unwrap();

        let recently = repo.was_dismissed_recently("alice:bob", 30).await.unwrap();
        assert!(recently, "should detect recent dismissal");

        let not_dismissed = repo.was_dismissed_recently("other:pair", 30).await.unwrap();
        assert!(
            !not_dismissed,
            "untouched pair should not be dismissed recently"
        );
    }
}
