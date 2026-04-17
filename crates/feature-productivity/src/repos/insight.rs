use common::time::bridge::jiff_to_chrono;
use sqlx::SqlitePool;

use crate::types::{InsightCard, InsightType, Sentiment};

#[derive(sqlx::FromRow)]
struct InsightRow {
    id: String,
    insight_type: String,
    title: String,
    body: String,
    sentiment: String,
    metric_value: Option<f64>,
    baseline_value: Option<f64>,
    date: String,
    dismissed: bool,
    generated_at: String,
}

impl From<InsightRow> for InsightCard {
    fn from(row: InsightRow) -> Self {
        Self {
            id: row.id,
            insight_type: row
                .insight_type
                .parse()
                .unwrap_or(InsightType::ConsistencyNote),
            title: row.title,
            body: row.body,
            sentiment: row.sentiment.parse().unwrap_or(Sentiment::Neutral),
            metric_value: row.metric_value,
            baseline_value: row.baseline_value,
            date: row.date,
            dismissed: row.dismissed,
            generated_at: common::parse_datetime(&row.generated_at, "UTC")
                .unwrap_or_else(|| jiff_to_chrono(jiff::Timestamp::now())),
        }
    }
}

#[derive(Debug, Clone)]
pub struct InsightRepo {
    pool: SqlitePool,
}

impl InsightRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn upsert(&self, card: &InsightCard) -> common::Result<()> {
        sqlx::query(
            r#"INSERT INTO insight_cards (id, insight_type, title, body, sentiment, metric_value, baseline_value, date, dismissed, generated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
               ON CONFLICT(id) DO UPDATE SET
                   title = excluded.title,
                   body = excluded.body,
                   sentiment = excluded.sentiment,
                   metric_value = excluded.metric_value,
                   baseline_value = excluded.baseline_value"#,
        )
        .bind(&card.id)
        .bind(card.insight_type.to_string())
        .bind(&card.title)
        .bind(&card.body)
        .bind(card.sentiment.to_string())
        .bind(card.metric_value)
        .bind(card.baseline_value)
        .bind(&card.date)
        .bind(card.dismissed)
        .bind(card.generated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn exists_for_date(
        &self,
        insight_type: InsightType,
        date: &str,
    ) -> common::Result<bool> {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM insight_cards WHERE insight_type = ?1 AND date = ?2)",
        )
        .bind(insight_type.to_string())
        .bind(date)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(exists)
    }

    pub async fn list_for_date(&self, date: &str) -> common::Result<Vec<InsightCard>> {
        let rows = sqlx::query_as::<_, InsightRow>(
            "SELECT id, insight_type, title, body, sentiment, metric_value, baseline_value, date, dismissed, generated_at FROM insight_cards WHERE date = ?1 ORDER BY generated_at DESC",
        )
        .bind(date)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows.into_iter().map(InsightCard::from).collect())
    }

    pub async fn dismiss(&self, id: &str) -> common::Result<()> {
        sqlx::query("UPDATE insight_cards SET dismissed = TRUE WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn list_undismissed(&self, limit: i64) -> common::Result<Vec<InsightCard>> {
        let rows = sqlx::query_as::<_, InsightRow>(
            "SELECT id, insight_type, title, body, sentiment, metric_value, baseline_value, date, dismissed, generated_at FROM insight_cards WHERE dismissed = FALSE ORDER BY generated_at DESC LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows.into_iter().map(InsightCard::from).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProductivityFeature;
    use common::time::bridge::jiff_to_chrono;

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
    async fn test_upsert_and_query() {
        let pool = setup_pool().await;
        let repo = InsightRepo::new(pool);

        let card = InsightCard {
            id: "deep-work-2026-03-06".to_string(),
            insight_type: InsightType::DeepWorkTrend,
            title: "Great deep work day!".to_string(),
            body: "You had 4 deep work blocks today, up from your average of 2.".to_string(),
            sentiment: Sentiment::Positive,
            metric_value: Some(4.0),
            baseline_value: Some(2.0),
            date: "2026-03-06".to_string(),
            dismissed: false,
            generated_at: jiff_to_chrono(jiff::Timestamp::now()),
        };

        repo.upsert(&card).await.unwrap();

        let results = repo.list_for_date("2026-03-06").await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Great deep work day!");
        assert_eq!(results[0].insight_type, InsightType::DeepWorkTrend);
    }

    #[tokio::test]
    async fn test_exists_for_date() {
        let pool = setup_pool().await;
        let repo = InsightRepo::new(pool);

        assert!(!repo
            .exists_for_date(InsightType::DeepWorkTrend, "2026-03-06")
            .await
            .unwrap());

        let card = InsightCard {
            id: "deep-work-2026-03-06".to_string(),
            insight_type: InsightType::DeepWorkTrend,
            title: "Test".to_string(),
            body: "Test body".to_string(),
            sentiment: Sentiment::Positive,
            metric_value: None,
            baseline_value: None,
            date: "2026-03-06".to_string(),
            dismissed: false,
            generated_at: jiff_to_chrono(jiff::Timestamp::now()),
        };
        repo.upsert(&card).await.unwrap();

        assert!(repo
            .exists_for_date(InsightType::DeepWorkTrend, "2026-03-06")
            .await
            .unwrap());
        assert!(!repo
            .exists_for_date(InsightType::DistractionSpike, "2026-03-06")
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn test_dismiss() {
        let pool = setup_pool().await;
        let repo = InsightRepo::new(pool);

        let card = InsightCard {
            id: "test-1".to_string(),
            insight_type: InsightType::StreakAchieved,
            title: "5 day streak!".to_string(),
            body: "Great consistency!".to_string(),
            sentiment: Sentiment::Positive,
            metric_value: Some(5.0),
            baseline_value: None,
            date: "2026-03-06".to_string(),
            dismissed: false,
            generated_at: jiff_to_chrono(jiff::Timestamp::now()),
        };
        repo.upsert(&card).await.unwrap();

        let undismissed = repo.list_undismissed(10).await.unwrap();
        assert_eq!(undismissed.len(), 1);

        repo.dismiss("test-1").await.unwrap();

        let undismissed = repo.list_undismissed(10).await.unwrap();
        assert_eq!(undismissed.len(), 0);
    }

    #[tokio::test]
    async fn test_upsert_idempotent() {
        let pool = setup_pool().await;
        let repo = InsightRepo::new(pool);

        let card = InsightCard {
            id: "test-idem".to_string(),
            insight_type: InsightType::FatigueWarning,
            title: "Watch out".to_string(),
            body: "You seem tired".to_string(),
            sentiment: Sentiment::Warning,
            metric_value: None,
            baseline_value: None,
            date: "2026-03-06".to_string(),
            dismissed: false,
            generated_at: jiff_to_chrono(jiff::Timestamp::now()),
        };
        repo.upsert(&card).await.unwrap();

        // Upsert with updated title
        let updated = InsightCard {
            title: "Updated warning".to_string(),
            ..card
        };
        repo.upsert(&updated).await.unwrap();

        let results = repo.list_for_date("2026-03-06").await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Updated warning");
    }
}
