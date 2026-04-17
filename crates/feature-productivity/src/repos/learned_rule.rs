use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tracing::warn;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearnedRule {
    pub id: Option<i64>,
    pub pattern: String,
    pub pattern_type: String,
    pub classification: String,
    pub confidence: f64,
    pub hit_count: i64,
    pub last_used_at: Timestamp,
    pub created_at: Timestamp,
}

#[derive(sqlx::FromRow)]
struct LearnedRuleRow {
    id: Option<i64>,
    pattern: String,
    pattern_type: String,
    classification: String,
    confidence: f64,
    hit_count: i64,
    last_used_at: String,
    created_at: String,
}

impl From<LearnedRuleRow> for LearnedRule {
    fn from(row: LearnedRuleRow) -> Self {
        let last_used_at = row
            .last_used_at
            .parse::<Timestamp>()
            .unwrap_or_else(|_| {
                warn!(raw = %row.last_used_at, "unparseable last_used_at in distraction_learned_rules");
                Timestamp::now()
            });
        let created_at = row
            .created_at
            .parse::<Timestamp>()
            .unwrap_or_else(|_| {
                warn!(raw = %row.created_at, "unparseable created_at in distraction_learned_rules");
                Timestamp::now()
            });
        Self {
            id: row.id,
            pattern: row.pattern,
            pattern_type: row.pattern_type,
            classification: row.classification,
            confidence: row.confidence,
            hit_count: row.hit_count,
            last_used_at,
            created_at,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LearnedRuleRepo {
    pool: SqlitePool,
}

impl LearnedRuleRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list_all(&self) -> common::Result<Vec<LearnedRule>> {
        let rows = sqlx::query_as::<_, LearnedRuleRow>(
            "SELECT * FROM distraction_learned_rules ORDER BY hit_count DESC LIMIT 500",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows.into_iter().map(LearnedRule::from).collect())
    }

    pub async fn find_by_pattern(
        &self,
        pattern: &str,
        pattern_type: &str,
    ) -> common::Result<Option<LearnedRule>> {
        let row = sqlx::query_as::<_, LearnedRuleRow>(
            "SELECT * FROM distraction_learned_rules WHERE pattern = ? AND pattern_type = ?",
        )
        .bind(pattern)
        .bind(pattern_type)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(row.map(LearnedRule::from))
    }

    pub async fn insert(&self, rule: &LearnedRule) -> common::Result<i64> {
        let id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO distraction_learned_rules (pattern, pattern_type, classification, confidence, hit_count, last_used_at, created_at) VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(&rule.pattern)
        .bind(&rule.pattern_type)
        .bind(&rule.classification)
        .bind(rule.confidence)
        .bind(rule.hit_count)
        .bind(rule.last_used_at.to_string())
        .bind(rule.created_at.to_string())
        .fetch_one(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(id)
    }

    pub async fn record_hit(&self, id: i64) -> common::Result<()> {
        let now = Timestamp::now().to_string();
        sqlx::query(
            "UPDATE distraction_learned_rules SET hit_count = hit_count + 1, confidence = MIN(1.0, confidence + 0.1), last_used_at = ? WHERE id = ?",
        )
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    /// Insert a new rule or increment hit_count on existing match (atomic).
    pub async fn upsert_or_hit(
        &self,
        pattern: &str,
        pattern_type: &str,
        classification: &str,
    ) -> common::Result<()> {
        let now = Timestamp::now().to_string();
        sqlx::query(
            "INSERT INTO distraction_learned_rules (pattern, pattern_type, classification, confidence, hit_count, last_used_at, created_at)
             VALUES (?, ?, ?, 0.5, 1, ?, ?)
             ON CONFLICT(pattern, pattern_type) DO UPDATE SET
               hit_count = hit_count + 1,
               confidence = MIN(1.0, confidence + 0.1),
               last_used_at = excluded.last_used_at",
        )
        .bind(pattern)
        .bind(pattern_type)
        .bind(classification)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    /// List distraction rules with confidence above threshold and sufficient hits.
    pub async fn list_high_confidence(
        &self,
        min_confidence: f64,
        min_hits: i32,
    ) -> Result<Vec<LearnedRule>, sqlx::Error> {
        let rows = sqlx::query_as::<_, LearnedRuleRow>(
            "SELECT * FROM distraction_learned_rules
             WHERE confidence >= ?1 AND hit_count >= ?2
             ORDER BY confidence DESC",
        )
        .bind(min_confidence)
        .bind(min_hits)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(LearnedRule::from).collect())
    }

    pub async fn delete(&self, id: i64) -> common::Result<()> {
        sqlx::query("DELETE FROM distraction_learned_rules WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProductivityFeature;

    async fn setup_pool() -> SqlitePool {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let inner = pool.inner().clone();
        storage::StoragePool::run_feature_migrations(
            &inner,
            &ProductivityFeature::migrations_static(),
        )
        .await
        .unwrap();
        inner
    }

    #[tokio::test]
    async fn insert_and_find() {
        let pool = setup_pool().await;
        let repo = LearnedRuleRepo::new(pool);
        let now = Timestamp::now();
        let rule = LearnedRule {
            id: None,
            pattern: "react tutorial".into(),
            pattern_type: "title_keyword".into(),
            classification: "educational".into(),
            confidence: 0.5,
            hit_count: 1,
            last_used_at: now,
            created_at: now,
        };
        let id = repo.insert(&rule).await.unwrap();
        assert!(id > 0);

        let found = repo
            .find_by_pattern("react tutorial", "title_keyword")
            .await
            .unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().classification, "educational");
    }

    #[tokio::test]
    async fn record_hit_increments() {
        let pool = setup_pool().await;
        let repo = LearnedRuleRepo::new(pool);
        let now = Timestamp::now();
        let rule = LearnedRule {
            id: None,
            pattern: "rust docs".into(),
            pattern_type: "title_keyword".into(),
            classification: "work_research".into(),
            confidence: 0.5,
            hit_count: 1,
            last_used_at: now,
            created_at: now,
        };
        let id = repo.insert(&rule).await.unwrap();
        repo.record_hit(id).await.unwrap();

        let found = repo
            .find_by_pattern("rust docs", "title_keyword")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.hit_count, 2);
        assert!((found.confidence - 0.6).abs() < 0.01);
    }

    #[tokio::test]
    async fn delete_removes_rule() {
        let pool = setup_pool().await;
        let repo = LearnedRuleRepo::new(pool);
        let now = Timestamp::now();
        let rule = LearnedRule {
            id: None,
            pattern: "temp".into(),
            pattern_type: "app_name".into(),
            classification: "educational".into(),
            confidence: 0.5,
            hit_count: 1,
            last_used_at: now,
            created_at: now,
        };
        let id = repo.insert(&rule).await.unwrap();
        repo.delete(id).await.unwrap();
        assert!(repo
            .find_by_pattern("temp", "app_name")
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn list_all_ordered_by_hit_count() {
        let pool = setup_pool().await;
        let repo = LearnedRuleRepo::new(pool);
        let now = Timestamp::now();

        let low = LearnedRule {
            id: None,
            pattern: "a".into(),
            pattern_type: "title_keyword".into(),
            classification: "educational".into(),
            confidence: 0.5,
            hit_count: 1,
            last_used_at: now,
            created_at: now,
        };
        let high = LearnedRule {
            id: None,
            pattern: "b".into(),
            pattern_type: "title_keyword".into(),
            classification: "educational".into(),
            confidence: 0.9,
            hit_count: 10,
            last_used_at: now,
            created_at: now,
        };
        repo.insert(&low).await.unwrap();
        repo.insert(&high).await.unwrap();

        let all = repo.list_all().await.unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].pattern, "b");
    }
}
