use sqlx::SqlitePool;

use crate::types::ActivityBucket;

#[derive(Debug, Clone)]
pub struct BucketRepo {
    pool: SqlitePool,
}

impl BucketRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn upsert(&self, bucket: &ActivityBucket) -> common::Result<()> {
        sqlx::query(
            r#"INSERT INTO activity_buckets (bucket_start, date, dominant_app, dominant_site, dominant_category, productive_secs, neutral_secs, distracting_secs, idle_secs, context_switches, focus_depth, tick_count)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
               ON CONFLICT(bucket_start) DO UPDATE SET
                   dominant_app = excluded.dominant_app,
                   dominant_site = excluded.dominant_site,
                   dominant_category = excluded.dominant_category,
                   productive_secs = excluded.productive_secs,
                   neutral_secs = excluded.neutral_secs,
                   distracting_secs = excluded.distracting_secs,
                   idle_secs = excluded.idle_secs,
                   context_switches = excluded.context_switches,
                   focus_depth = excluded.focus_depth,
                   tick_count = excluded.tick_count"#,
        )
        .bind(&bucket.bucket_start)
        .bind(&bucket.date)
        .bind(&bucket.dominant_app)
        .bind(&bucket.dominant_site)
        .bind(&bucket.dominant_category)
        .bind(bucket.productive_secs)
        .bind(bucket.neutral_secs)
        .bind(bucket.distracting_secs)
        .bind(bucket.idle_secs)
        .bind(bucket.context_switches)
        .bind(bucket.focus_depth)
        .bind(bucket.tick_count)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn list_range(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> common::Result<Vec<ActivityBucket>> {
        let rows = sqlx::query_as::<_, ActivityBucket>(
            r#"SELECT bucket_start, date, dominant_app, dominant_site, dominant_category,
                      productive_secs, neutral_secs, distracting_secs, idle_secs,
                      context_switches, focus_depth, tick_count
               FROM activity_buckets
               WHERE date >= ?1 AND date <= ?2
               ORDER BY bucket_start ASC"#,
        )
        .bind(start_date)
        .bind(end_date)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows)
    }

    pub async fn purge_before(&self, before_date: &str) -> common::Result<u64> {
        let result = sqlx::query("DELETE FROM activity_buckets WHERE date < ?1")
            .bind(before_date)
            .execute(&self.pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(result.rows_affected())
    }

    pub async fn aggregate_day(
        &self,
        date: &str,
    ) -> common::Result<Option<(i64, i64, i64, i64, i64)>> {
        #[derive(sqlx::FromRow)]
        struct Row {
            productive: i64,
            neutral: i64,
            distracting: i64,
            idle: i64,
            switches: i64,
        }
        let row = sqlx::query_as::<_, Row>(
            r#"SELECT COALESCE(SUM(productive_secs), 0) as productive,
                      COALESCE(SUM(neutral_secs), 0) as neutral,
                      COALESCE(SUM(distracting_secs), 0) as distracting,
                      COALESCE(SUM(idle_secs), 0) as idle,
                      COALESCE(SUM(context_switches), 0) as switches
               FROM activity_buckets WHERE date = ?1"#,
        )
        .bind(date)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(row.map(|r| (r.productive, r.neutral, r.distracting, r.idle, r.switches)))
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
    async fn test_upsert_and_query() {
        let pool = setup_pool().await;
        let repo = BucketRepo::new(pool);

        let bucket = ActivityBucket {
            bucket_start: "2026-03-06T10:00:00+00:00".to_string(),
            date: "2026-03-06".to_string(),
            dominant_app: Some("VS Code".to_string()),
            dominant_site: None,
            dominant_category: Some("coding".to_string()),
            productive_secs: 240,
            neutral_secs: 30,
            distracting_secs: 30,
            idle_secs: 0,
            context_switches: 2,
            focus_depth: Some(0.8),
            tick_count: 60,
        };

        repo.upsert(&bucket).await.unwrap();

        let results = repo.list_range("2026-03-06", "2026-03-06").await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].productive_secs, 240);
        assert_eq!(results[0].dominant_app.as_deref(), Some("VS Code"));
    }

    #[tokio::test]
    async fn test_upsert_idempotent() {
        let pool = setup_pool().await;
        let repo = BucketRepo::new(pool);

        let bucket = ActivityBucket {
            bucket_start: "2026-03-06T10:00:00+00:00".to_string(),
            date: "2026-03-06".to_string(),
            dominant_app: Some("VS Code".to_string()),
            dominant_site: None,
            dominant_category: None,
            productive_secs: 100,
            neutral_secs: 0,
            distracting_secs: 0,
            idle_secs: 0,
            context_switches: 0,
            focus_depth: None,
            tick_count: 20,
        };

        repo.upsert(&bucket).await.unwrap();

        // Upsert again with updated values
        let updated = ActivityBucket {
            productive_secs: 200,
            tick_count: 40,
            ..bucket
        };
        repo.upsert(&updated).await.unwrap();

        let results = repo.list_range("2026-03-06", "2026-03-06").await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].productive_secs, 200);
        assert_eq!(results[0].tick_count, 40);
    }

    #[tokio::test]
    async fn test_aggregate_day() {
        let pool = setup_pool().await;
        let repo = BucketRepo::new(pool);

        for i in 0..3 {
            let bucket = ActivityBucket {
                bucket_start: format!("2026-03-06T{:02}:00:00+00:00", 10 + i),
                date: "2026-03-06".to_string(),
                dominant_app: None,
                dominant_site: None,
                dominant_category: None,
                productive_secs: 100,
                neutral_secs: 50,
                distracting_secs: 25,
                idle_secs: 25,
                context_switches: 2,
                focus_depth: None,
                tick_count: 60,
            };
            repo.upsert(&bucket).await.unwrap();
        }

        let agg = repo.aggregate_day("2026-03-06").await.unwrap().unwrap();
        assert_eq!(agg.0, 300); // productive
        assert_eq!(agg.1, 150); // neutral
        assert_eq!(agg.2, 75); // distracting
        assert_eq!(agg.3, 75); // idle
        assert_eq!(agg.4, 6); // switches
    }

    #[tokio::test]
    async fn test_purge_before() {
        let pool = setup_pool().await;
        let repo = BucketRepo::new(pool);

        for date in &["2026-03-01", "2026-03-05", "2026-03-06"] {
            let bucket = ActivityBucket {
                bucket_start: format!("{}T10:00:00+00:00", date),
                date: date.to_string(),
                dominant_app: None,
                dominant_site: None,
                dominant_category: None,
                productive_secs: 100,
                neutral_secs: 0,
                distracting_secs: 0,
                idle_secs: 0,
                context_switches: 0,
                focus_depth: None,
                tick_count: 20,
            };
            repo.upsert(&bucket).await.unwrap();
        }

        let purged = repo.purge_before("2026-03-05").await.unwrap();
        assert_eq!(purged, 1); // only 2026-03-01 purged

        let remaining = repo.list_range("2026-01-01", "2026-12-31").await.unwrap();
        assert_eq!(remaining.len(), 2);
    }
}
