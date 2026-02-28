//! Repository for the `agent_tasks` table — subagent coordination task board.

use crate::error::OptionExt;
use crate::rows::agent_task::AgentTaskRow;
use crate::StorageError;
use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct AgentTaskRepo {
    pool: SqlitePool,
}

impl AgentTaskRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        session_key: &str,
        description: &str,
        blocked_by: &[String],
    ) -> Result<AgentTaskRow, StorageError> {
        let id = Uuid::new_v4().to_string();
        let blocked_by_json =
            serde_json::to_string(blocked_by).unwrap_or_else(|_| "[]".to_string());

        sqlx::query_as::<_, AgentTaskRow>(
            "INSERT INTO agent_tasks (id, session_key, description, blocked_by)
             VALUES (?1, ?2, ?3, ?4)
             RETURNING *",
        )
        .bind(&id)
        .bind(session_key)
        .bind(description)
        .bind(&blocked_by_json)
        .fetch_one(&self.pool)
        .await
        .map_err(StorageError::from)
    }

    pub async fn claim(&self, task_id: &str, agent_id: &str) -> Result<AgentTaskRow, StorageError> {
        let now = Utc::now();
        sqlx::query_as::<_, AgentTaskRow>(
            "UPDATE agent_tasks
             SET owner_agent_id = ?1, status = 'claimed', updated_at = ?3
             WHERE id = ?2 AND owner_agent_id IS NULL AND status = 'pending'
             RETURNING *",
        )
        .bind(agent_id)
        .bind(task_id)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| {
            StorageError::NotFound(format!("Task {} not found or already claimed", task_id))
        })
    }

    pub async fn update_status(
        &self,
        task_id: &str,
        status: &str,
        result: Option<&str>,
        error: Option<&str>,
    ) -> Result<AgentTaskRow, StorageError> {
        let now = Utc::now();
        sqlx::query_as::<_, AgentTaskRow>(
            "UPDATE agent_tasks
             SET status = ?1, result = ?2, error = ?3, updated_at = ?5
             WHERE id = ?4
             RETURNING *",
        )
        .bind(status)
        .bind(result)
        .bind(error)
        .bind(task_id)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_not_found(&format!("Task {} not found", task_id))
    }

    pub async fn list_by_session(
        &self,
        session_key: &str,
    ) -> Result<Vec<AgentTaskRow>, StorageError> {
        sqlx::query_as::<_, AgentTaskRow>(
            "SELECT * FROM agent_tasks WHERE session_key = ?1 ORDER BY created_at",
        )
        .bind(session_key)
        .fetch_all(&self.pool)
        .await
        .map_err(StorageError::from)
    }

    pub async fn list_available(
        &self,
        session_key: &str,
    ) -> Result<Vec<AgentTaskRow>, StorageError> {
        // Available = pending, unclaimed, and all blocked_by tasks are completed.
        // First fetch only pending+unclaimed rows (avoids loading completed/failed rows).
        let pending: Vec<AgentTaskRow> = sqlx::query_as::<_, AgentTaskRow>(
            "SELECT * FROM agent_tasks
             WHERE session_key = ?1 AND status = 'pending' AND owner_agent_id IS NULL
             ORDER BY created_at",
        )
        .bind(session_key)
        .fetch_all(&self.pool)
        .await?;

        // For tasks with dependencies, check if all blockers are completed.
        // Only fetch completed IDs (not full rows) for the dependency check.
        let completed_ids: std::collections::HashSet<String> = sqlx::query_scalar::<_, String>(
            "SELECT id FROM agent_tasks WHERE session_key = ?1 AND status = 'completed'",
        )
        .bind(session_key)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .collect();

        Ok(pending
            .into_iter()
            .filter(|t| {
                let blocked: Vec<String> = serde_json::from_str(&t.blocked_by).unwrap_or_default();
                blocked.iter().all(|id| completed_ids.contains(id))
            })
            .collect())
    }

    pub async fn delete_by_session(&self, session_key: &str) -> Result<u64, StorageError> {
        let result = sqlx::query("DELETE FROM agent_tasks WHERE session_key = ?1")
            .bind(session_key)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    pub async fn get(&self, task_id: &str) -> Result<AgentTaskRow, StorageError> {
        sqlx::query_as::<_, AgentTaskRow>("SELECT * FROM agent_tasks WHERE id = ?1")
            .bind(task_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_not_found(&format!("Task {} not found", task_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StoragePool;

    async fn setup() -> AgentTaskRepo {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        AgentTaskRepo::new(pool.inner().clone())
    }

    #[tokio::test]
    async fn test_create_and_get() {
        let repo = setup().await;
        let task = repo
            .create("sess:1", "Do research", &[] as &[String])
            .await
            .unwrap();
        assert_eq!(task.status, "pending");
        assert_eq!(task.description, "Do research");
        assert!(task.owner_agent_id.is_none());

        let fetched = repo.get(&task.id).await.unwrap();
        assert_eq!(fetched.id, task.id);
    }

    #[tokio::test]
    async fn test_claim() {
        let repo = setup().await;
        let task = repo
            .create("sess:1", "Research", &[] as &[String])
            .await
            .unwrap();

        let claimed = repo.claim(&task.id, "agent-abc").await.unwrap();
        assert_eq!(claimed.status, "claimed");
        assert_eq!(claimed.owner_agent_id.as_deref(), Some("agent-abc"));

        // Double claim should fail
        let err = repo.claim(&task.id, "agent-xyz").await;
        assert!(err.is_err());
    }

    #[tokio::test]
    async fn test_update_status_complete() {
        let repo = setup().await;
        let task = repo
            .create("sess:1", "Work", &[] as &[String])
            .await
            .unwrap();
        repo.claim(&task.id, "agent-1").await.unwrap();

        let updated = repo
            .update_status(&task.id, "completed", Some("Done!"), None)
            .await
            .unwrap();
        assert_eq!(updated.status, "completed");
        assert_eq!(updated.result.as_deref(), Some("Done!"));
    }

    #[tokio::test]
    async fn test_list_available_respects_blocking() {
        let repo = setup().await;
        let t1 = repo
            .create("sess:1", "First", &[] as &[String])
            .await
            .unwrap();
        let _t2 = repo
            .create("sess:1", "Second (blocked)", std::slice::from_ref(&t1.id))
            .await
            .unwrap();
        let t3 = repo
            .create("sess:1", "Third (free)", &[] as &[String])
            .await
            .unwrap();

        let available = repo.list_available("sess:1").await.unwrap();
        assert_eq!(available.len(), 2); // t1 and t3 are available
        let ids: Vec<&str> = available.iter().map(|t| t.id.as_str()).collect();
        assert!(ids.contains(&t1.id.as_str()));
        assert!(ids.contains(&t3.id.as_str()));

        // Complete t1, now t2 should become available
        repo.claim(&t1.id, "a").await.unwrap();
        repo.update_status(&t1.id, "completed", None, None)
            .await
            .unwrap();
        let available2 = repo.list_available("sess:1").await.unwrap();
        assert_eq!(available2.len(), 2); // t2 and t3
    }

    #[tokio::test]
    async fn test_delete_by_session() {
        let repo = setup().await;
        repo.create("sess:1", "A", &[] as &[String]).await.unwrap();
        repo.create("sess:1", "B", &[] as &[String]).await.unwrap();
        repo.create("sess:2", "C", &[] as &[String]).await.unwrap();

        let deleted = repo.delete_by_session("sess:1").await.unwrap();
        assert_eq!(deleted, 2);

        let remaining = repo.list_by_session("sess:2").await.unwrap();
        assert_eq!(remaining.len(), 1);
    }
}
