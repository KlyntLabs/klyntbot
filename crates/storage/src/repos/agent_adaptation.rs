//! Repository for the `agent_adaptations` table.

use sqlx::SqlitePool;

use crate::error::StorageError;
use crate::rows::learning::AgentAdaptationRow;

/// Repository for per-agent user preference adaptations.
#[derive(Debug, Clone)]
pub struct AgentAdaptationRepo {
    pool: SqlitePool,
}

impl AgentAdaptationRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Upsert an agent adaptation.
    pub async fn upsert(
        &self,
        agent_name: &str,
        preference_key: &str,
        preference_value: &serde_json::Value,
        source: &str,
        confidence: f64,
    ) -> Result<AgentAdaptationRow, StorageError> {
        let value_str = serde_json::to_string(preference_value).unwrap_or_default();
        let row = sqlx::query_as::<_, AgentAdaptationRow>(
            r#"
            INSERT INTO agent_adaptations (agent_name, preference_key, preference_value, source, confidence)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT (agent_name, preference_key)
            DO UPDATE SET
                preference_value = ?3,
                source = ?4,
                confidence = ?5,
                last_updated = datetime('now')
            RETURNING *
            "#,
        )
        .bind(agent_name)
        .bind(preference_key)
        .bind(&value_str)
        .bind(source)
        .bind(confidence)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// List adaptations for a specific agent.
    pub async fn list_by_agent(
        &self,
        agent_name: &str,
    ) -> Result<Vec<AgentAdaptationRow>, StorageError> {
        let rows = sqlx::query_as::<_, AgentAdaptationRow>(
            "SELECT * FROM agent_adaptations WHERE agent_name = ?1 ORDER BY confidence DESC",
        )
        .bind(agent_name)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// List all adaptations.
    pub async fn list_all(&self) -> Result<Vec<AgentAdaptationRow>, StorageError> {
        let rows = sqlx::query_as::<_, AgentAdaptationRow>(
            "SELECT * FROM agent_adaptations ORDER BY agent_name, preference_key",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StoragePool;

    #[tokio::test]
    async fn test_upsert_and_list() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = AgentAdaptationRepo::new(pool.inner().clone());

        repo.upsert(
            "task",
            "response_length",
            &serde_json::json!("concise"),
            "satisfaction_signal",
            0.8,
        )
        .await
        .unwrap();

        let adaptations = repo.list_by_agent("task").await.unwrap();
        assert_eq!(adaptations.len(), 1);
        assert_eq!(adaptations[0].preference_key, "response_length");
    }

    #[tokio::test]
    async fn test_upsert_updates_existing() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = AgentAdaptationRepo::new(pool.inner().clone());

        repo.upsert("task", "style", &serde_json::json!("formal"), "signal", 0.5)
            .await
            .unwrap();
        repo.upsert("task", "style", &serde_json::json!("casual"), "signal", 0.9)
            .await
            .unwrap();

        let adaptations = repo.list_by_agent("task").await.unwrap();
        assert_eq!(adaptations.len(), 1);
        assert_eq!(adaptations[0].confidence, 0.9);
    }
}
