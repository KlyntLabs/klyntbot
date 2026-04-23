use ai_core::{Aggregation, MetricSpec};
use sqlx::SqlitePool;

/// Repository for the unified `ai_metric_samples` table.
#[derive(Clone)]
pub struct MetricRepo {
    pool: SqlitePool,
}

impl MetricRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert a single metric sample.
    pub async fn insert_sample(
        &self,
        metric_name: &str,
        value: f64,
        sample_time: i64,
        source_domain: &str,
        source_event_kind: &str,
    ) -> common::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO ai_metric_samples (metric_name, value, sample_time, source_domain, source_event_kind)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(metric_name)
        .bind(value)
        .bind(sample_time)
        .bind(source_domain)
        .bind(source_event_kind)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Aggregate a metric over its configured window.
    /// Returns `None` if fewer than `min_samples` exist.
    pub async fn aggregate_metric(
        &self,
        spec: &MetricSpec,
        now: i64,
    ) -> common::Result<Option<f64>> {
        let window_start = now - spec.window_secs as i64;
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM ai_metric_samples WHERE metric_name = ? AND sample_time >= ?",
        )
        .bind(spec.name)
        .bind(window_start)
        .fetch_one(&self.pool)
        .await?;

        if count < spec.min_samples as i64 {
            return Ok(None);
        }

        let sql = format!(
            "SELECT {} FROM ai_metric_samples WHERE metric_name = ? AND sample_time >= ?",
            spec.aggregation.as_sql_expr()
        );
        let result: Option<f64> = sqlx::query_scalar(&sql)
            .bind(spec.name)
            .bind(window_start)
            .fetch_optional(&self.pool)
            .await?;

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ai_core::{Aggregation, MetricSample, MetricSpec};

    #[tokio::test]
    async fn insert_and_aggregate() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query(
            r#"
            CREATE TABLE ai_metric_samples (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                metric_name TEXT NOT NULL,
                value REAL NOT NULL,
                sample_time INTEGER NOT NULL,
                source_domain TEXT NOT NULL,
                source_event_kind TEXT NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        let repo = MetricRepo::new(pool);
        let now = jiff::Timestamp::now().as_second();

        repo.insert_sample("test_metric", 1.0, now, "tasks", "Created")
            .await
            .unwrap();
        repo.insert_sample("test_metric", 2.0, now - 100, "tasks", "Created")
            .await
            .unwrap();
        repo.insert_sample("test_metric", 3.0, now - 200, "tasks", "Created")
            .await
            .unwrap();

        let spec = MetricSpec {
            name: "test_metric",
            window_secs: 3600,
            min_samples: 2,
            aggregation: Aggregation::Avg,
        };

        let avg = repo.aggregate_metric(&spec, now).await.unwrap();
        assert!(avg.is_some());
        assert!((avg.unwrap() - 2.0).abs() < 0.01);
    }
}
