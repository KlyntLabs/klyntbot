//! Repository for the `coaching_intervention_log` table (cognitive migration v10).

use sqlx::SqlitePool;

use crate::error::StorageError;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct InterventionLogRow {
    pub id: String,
    pub intervention_type: String,
    pub message: String,
    pub trigger_name: String,
    pub feedback: Option<String>,
    pub delivered_at: String,
    pub feedback_at: Option<String>,
    pub action_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CoachingInterventionLogRepo {
    pool: SqlitePool,
}

impl CoachingInterventionLogRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn insert(
        &self,
        id: &str,
        intervention_type: &str,
        message: &str,
        trigger_name: &str,
        delivered_at: &str,
        action_url: Option<&str>,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT OR IGNORE INTO coaching_intervention_log
                (id, intervention_type, message, trigger_name, delivered_at, action_url)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(id)
        .bind(intervention_type)
        .bind(message)
        .bind(trigger_name)
        .bind(delivered_at)
        .bind(action_url)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_feedback(&self, id: &str, feedback: &str) -> Result<bool, StorageError> {
        let result = sqlx::query(
            "UPDATE coaching_intervention_log
             SET feedback = ?2, feedback_at = datetime('now')
             WHERE id = ?1",
        )
        .bind(id)
        .bind(feedback)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn list_recent(&self, limit: i64) -> Result<Vec<InterventionLogRow>, StorageError> {
        let rows = sqlx::query_as::<_, InterventionLogRow>(
            "SELECT id, intervention_type, message, trigger_name, feedback, delivered_at, feedback_at, action_url
             FROM coaching_intervention_log
             ORDER BY delivered_at DESC
             LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> CoachingInterventionLogRepo {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS coaching_intervention_log (
                id TEXT PRIMARY KEY,
                intervention_type TEXT NOT NULL,
                message TEXT NOT NULL,
                trigger_name TEXT NOT NULL,
                feedback TEXT,
                delivered_at TEXT NOT NULL,
                feedback_at TEXT,
                action_url TEXT
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        CoachingInterventionLogRepo::new(pool)
    }

    #[tokio::test]
    async fn test_insert_and_list() {
        let repo = setup().await;
        repo.insert(
            "int-1",
            "ChatMessage",
            "Take a break",
            "distraction_streak",
            "2026-03-21T10:00:00Z",
            None,
        )
        .await
        .unwrap();

        let rows = repo.list_recent(10).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "int-1");
        assert_eq!(rows[0].trigger_name, "distraction_streak");
        assert!(rows[0].feedback.is_none());
    }

    #[tokio::test]
    async fn test_update_feedback() {
        let repo = setup().await;
        repo.insert(
            "int-2",
            "DashboardCard",
            "Focus now",
            "overdue_pressure",
            "2026-03-21T11:00:00Z",
            None,
        )
        .await
        .unwrap();

        let updated = repo.update_feedback("int-2", "helpful").await.unwrap();
        assert!(updated);

        let rows = repo.list_recent(10).await.unwrap();
        assert_eq!(rows[0].feedback.as_deref(), Some("helpful"));
        assert!(rows[0].feedback_at.is_some());
    }

    #[tokio::test]
    async fn test_update_feedback_nonexistent() {
        let repo = setup().await;
        let updated = repo.update_feedback("no-such-id", "helpful").await.unwrap();
        assert!(!updated);
    }

    #[tokio::test]
    async fn test_insert_duplicate_is_ignored() {
        let repo = setup().await;
        repo.insert(
            "dup-1",
            "ChatMessage",
            "msg1",
            "trigger1",
            "2026-03-21T12:00:00Z",
            None,
        )
        .await
        .unwrap();
        repo.insert(
            "dup-1",
            "ChatMessage",
            "msg2",
            "trigger2",
            "2026-03-21T13:00:00Z",
            None,
        )
        .await
        .unwrap();

        let rows = repo.list_recent(10).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].message, "msg1");
    }
}
