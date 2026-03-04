//! Repository for the `behavioral_patterns` table.

use sqlx::SqlitePool;

use crate::error::StorageError;
use crate::rows::learning::BehavioralPatternRow;

/// Repository for observed behavioral patterns.
#[derive(Debug, Clone)]
pub struct BehavioralPatternRepo {
    pool: SqlitePool,
}

impl BehavioralPatternRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Upsert a behavioral pattern.
    pub async fn upsert(
        &self,
        pattern_type: &str,
        pattern_key: &str,
        pattern_value: &serde_json::Value,
        sample_count: i32,
    ) -> Result<BehavioralPatternRow, StorageError> {
        let value_str = serde_json::to_string(pattern_value).unwrap_or_default();
        let row = sqlx::query_as::<_, BehavioralPatternRow>(
            r#"
            INSERT INTO behavioral_patterns (pattern_type, pattern_key, pattern_value, sample_count)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT (pattern_type, pattern_key)
            DO UPDATE SET
                pattern_value = ?3,
                sample_count = ?4,
                last_updated = datetime('now')
            RETURNING *
            "#,
        )
        .bind(pattern_type)
        .bind(pattern_key)
        .bind(&value_str)
        .bind(sample_count)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// List patterns by type.
    pub async fn list_by_type(
        &self,
        pattern_type: &str,
    ) -> Result<Vec<BehavioralPatternRow>, StorageError> {
        let rows = sqlx::query_as::<_, BehavioralPatternRow>(
            "SELECT * FROM behavioral_patterns WHERE pattern_type = ?1 ORDER BY sample_count DESC",
        )
        .bind(pattern_type)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// List all patterns with a minimum sample count.
    pub async fn list_reliable(
        &self,
        min_samples: i32,
    ) -> Result<Vec<BehavioralPatternRow>, StorageError> {
        let rows = sqlx::query_as::<_, BehavioralPatternRow>(
            "SELECT * FROM behavioral_patterns WHERE sample_count >= ?1 ORDER BY sample_count DESC",
        )
        .bind(min_samples)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// List all patterns.
    pub async fn list_all(&self) -> Result<Vec<BehavioralPatternRow>, StorageError> {
        let rows = sqlx::query_as::<_, BehavioralPatternRow>(
            "SELECT * FROM behavioral_patterns ORDER BY pattern_type, pattern_key",
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
        let repo = BehavioralPatternRepo::new(pool.inner().clone());

        repo.upsert(
            "day_of_week",
            "monday_task",
            &serde_json::json!({"agent": "task", "day": "monday"}),
            15,
        )
        .await
        .unwrap();

        let patterns = repo.list_by_type("day_of_week").await.unwrap();
        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0].sample_count, 15);
    }

    #[tokio::test]
    async fn test_list_reliable() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = BehavioralPatternRepo::new(pool.inner().clone());

        repo.upsert("usage", "high", &serde_json::json!("task"), 20)
            .await
            .unwrap();
        repo.upsert("usage", "low", &serde_json::json!("calendar"), 3)
            .await
            .unwrap();

        let reliable = repo.list_reliable(10).await.unwrap();
        assert_eq!(reliable.len(), 1);
        assert_eq!(reliable[0].pattern_key, "high");
    }
}
