//! Repository for the `user_profile` table.

use sqlx::SqlitePool;

use crate::error::StorageError;
use crate::rows::learning::UserProfileRow;

/// Repository for user profile facts (explicit and inferred).
#[derive(Debug, Clone)]
pub struct UserProfileRepo {
    pool: SqlitePool,
}

impl UserProfileRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Upsert a user profile entry (insert or update on conflict).
    pub async fn upsert(
        &self,
        category: &str,
        key: &str,
        value: &serde_json::Value,
        source: &str,
        confidence: f64,
        agent_name: Option<&str>,
    ) -> Result<UserProfileRow, StorageError> {
        let value_str = serde_json::to_string(value).unwrap_or_default();
        let row = sqlx::query_as::<_, UserProfileRow>(
            r#"
            INSERT INTO user_profile (category, key, value, source, confidence, agent_name)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT (category, key)
            DO UPDATE SET
                value = ?3,
                source = ?4,
                confidence = ?5,
                agent_name = COALESCE(?6, user_profile.agent_name),
                last_confirmed = datetime('now')
            RETURNING *
            "#,
        )
        .bind(category)
        .bind(key)
        .bind(&value_str)
        .bind(source)
        .bind(confidence)
        .bind(agent_name)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// Get a single profile entry by category and key.
    pub async fn get(
        &self,
        category: &str,
        key: &str,
    ) -> Result<Option<UserProfileRow>, StorageError> {
        let row = sqlx::query_as::<_, UserProfileRow>(
            "SELECT * FROM user_profile WHERE category = ?1 AND key = ?2",
        )
        .bind(category)
        .bind(key)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// List all entries in a category.
    pub async fn list_by_category(
        &self,
        category: &str,
    ) -> Result<Vec<UserProfileRow>, StorageError> {
        let rows = sqlx::query_as::<_, UserProfileRow>(
            "SELECT * FROM user_profile WHERE category = ?1 ORDER BY key",
        )
        .bind(category)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// List entries with confidence above a threshold.
    pub async fn list_above_confidence(
        &self,
        min_confidence: f64,
    ) -> Result<Vec<UserProfileRow>, StorageError> {
        let rows = sqlx::query_as::<_, UserProfileRow>(
            "SELECT * FROM user_profile WHERE confidence >= ?1 ORDER BY confidence DESC",
        )
        .bind(min_confidence)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// List all entries (for context injection).
    pub async fn list_all(&self) -> Result<Vec<UserProfileRow>, StorageError> {
        let rows = sqlx::query_as::<_, UserProfileRow>(
            "SELECT * FROM user_profile ORDER BY category, key",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Delete a profile entry.
    pub async fn delete(&self, category: &str, key: &str) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM user_profile WHERE category = ?1 AND key = ?2")
            .bind(category)
            .bind(key)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StoragePool;

    #[tokio::test]
    async fn test_upsert_and_get() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = UserProfileRepo::new(pool.inner().clone());

        repo.upsert(
            "projects",
            "active_project",
            &serde_json::json!("Project X"),
            "user_explicit",
            1.0,
            None,
        )
        .await
        .unwrap();
        let entry = repo.get("projects", "active_project").await.unwrap();
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert_eq!(
            entry.value,
            serde_json::to_string(&serde_json::json!("Project X")).unwrap()
        );
    }

    #[tokio::test]
    async fn test_list_by_category() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = UserProfileRepo::new(pool.inner().clone());

        repo.upsert(
            "preferences",
            "timezone",
            &serde_json::json!("EST"),
            "user_explicit",
            1.0,
            None,
        )
        .await
        .unwrap();
        repo.upsert(
            "preferences",
            "language",
            &serde_json::json!("en"),
            "user_explicit",
            1.0,
            None,
        )
        .await
        .unwrap();

        let entries = repo.list_by_category("preferences").await.unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[tokio::test]
    async fn test_list_high_confidence() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = UserProfileRepo::new(pool.inner().clone());

        repo.upsert(
            "habits",
            "morning_routine",
            &serde_json::json!("coffee"),
            "system_inferred",
            0.3,
            None,
        )
        .await
        .unwrap();
        repo.upsert(
            "habits",
            "finance_day",
            &serde_json::json!("friday"),
            "system_inferred",
            0.8,
            None,
        )
        .await
        .unwrap();

        let entries = repo.list_above_confidence(0.5).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].key, "finance_day");
    }

    #[tokio::test]
    async fn test_upsert_updates_existing() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = UserProfileRepo::new(pool.inner().clone());

        repo.upsert(
            "ctx",
            "name",
            &serde_json::json!("Alice"),
            "user_explicit",
            1.0,
            None,
        )
        .await
        .unwrap();
        repo.upsert(
            "ctx",
            "name",
            &serde_json::json!("Bob"),
            "agent_observed",
            0.9,
            Some("general"),
        )
        .await
        .unwrap();

        let entry = repo.get("ctx", "name").await.unwrap().unwrap();
        assert_eq!(
            entry.value,
            serde_json::to_string(&serde_json::json!("Bob")).unwrap()
        );
        assert_eq!(entry.source, "agent_observed");
        assert_eq!(entry.agent_name.as_deref(), Some("general"));
    }
}
