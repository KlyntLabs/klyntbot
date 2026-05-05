//! Repository for the `coaching_strategies` table (cognitive migration 001).

use sqlx::SqlitePool;

use crate::error::StorageError;

/// Row struct for coaching_strategies table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CoachingStrategyRow {
    pub id: String,
    pub strategy_type: String,
    pub domain: String,
    pub times_used: i32,
    pub times_accepted: i32,
    pub times_led_to_improvement: i32,
    pub avg_improvement_magnitude: Option<f64>,
    pub confidence: f64,
    pub last_used: Option<String>,
    pub created_at: String,
}

/// Input for upserting a coaching strategy by (strategy_type, domain).
pub struct UpsertCoachingStrategy<'a> {
    pub strategy_type: &'a str,
    pub domain: &'a str,
    pub times_used: i32,
    pub times_accepted: i32,
    pub times_led_to_improvement: i32,
    pub avg_improvement_magnitude: Option<f64>,
    pub confidence: f64,
}

/// Repository for coaching strategy effectiveness tracking.
#[derive(Debug, Clone)]
pub struct CoachingStrategyRepo {
    pool: SqlitePool,
}

impl CoachingStrategyRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Upsert by matching on (strategy_type, domain) instead of id.
    pub async fn upsert(&self, input: &UpsertCoachingStrategy<'_>) -> Result<(), StorageError> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO coaching_strategies (id, strategy_type, domain, times_used, times_accepted,
                times_led_to_improvement, avg_improvement_magnitude, confidence, last_used)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'))
             ON CONFLICT(strategy_type, domain) DO UPDATE SET
                times_used = excluded.times_used,
                times_accepted = excluded.times_accepted,
                times_led_to_improvement = excluded.times_led_to_improvement,
                avg_improvement_magnitude = excluded.avg_improvement_magnitude,
                confidence = excluded.confidence,
                last_used = excluded.last_used",
        )
        .bind(&id)
        .bind(input.strategy_type)
        .bind(input.domain)
        .bind(input.times_used)
        .bind(input.times_accepted)
        .bind(input.times_led_to_improvement)
        .bind(input.avg_improvement_magnitude)
        .bind(input.confidence)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Load all coaching strategies.
    pub async fn list_all(&self) -> Result<Vec<CoachingStrategyRow>, StorageError> {
        let rows = sqlx::query_as::<_, CoachingStrategyRow>(
            "SELECT * FROM coaching_strategies ORDER BY times_used DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> CoachingStrategyRepo {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        // Create the table (normally from cognitive migration)
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS coaching_strategies (
                id TEXT PRIMARY KEY,
                strategy_type TEXT NOT NULL,
                domain TEXT NOT NULL,
                times_used INTEGER NOT NULL DEFAULT 0,
                times_accepted INTEGER NOT NULL DEFAULT 0,
                times_led_to_improvement INTEGER NOT NULL DEFAULT 0,
                avg_improvement_magnitude REAL,
                confidence REAL NOT NULL DEFAULT 0.5,
                last_used TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(strategy_type, domain)
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        CoachingStrategyRepo::new(pool)
    }

    #[tokio::test]
    async fn test_upsert_and_list() {
        let repo = setup().await;
        repo.upsert(&UpsertCoachingStrategy {
            strategy_type: "distraction_streak",
            domain: "coaching",
            times_used: 5,
            times_accepted: 3,
            times_led_to_improvement: 2,
            avg_improvement_magnitude: Some(0.7),
            confidence: 0.8,
        })
        .await
        .unwrap();

        let rows = repo.list_all().await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].strategy_type, "distraction_streak");
        assert_eq!(rows[0].times_used, 5);
    }

    #[tokio::test]
    async fn test_upsert_updates_existing() {
        let repo = setup().await;
        repo.upsert(&UpsertCoachingStrategy {
            strategy_type: "test",
            domain: "coaching",
            times_used: 1,
            times_accepted: 0,
            times_led_to_improvement: 0,
            avg_improvement_magnitude: None,
            confidence: 0.5,
        })
        .await
        .unwrap();
        repo.upsert(&UpsertCoachingStrategy {
            strategy_type: "test",
            domain: "coaching",
            times_used: 5,
            times_accepted: 3,
            times_led_to_improvement: 1,
            avg_improvement_magnitude: Some(0.6),
            confidence: 0.75,
        })
        .await
        .unwrap();

        let rows = repo.list_all().await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].times_used, 5);
        assert_eq!(rows[0].times_accepted, 3);
    }
}
