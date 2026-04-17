//! Usage repository — usage_records table.

use sqlx::SqlitePool;

use crate::error::StorageError;
use crate::rows::usage::UsageRecordRow;

/// Repository for LLM usage record persistence and reporting.
#[derive(Debug, Clone)]
pub struct UsageRepo {
    pool: SqlitePool,
}

impl UsageRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Append a usage record.
    pub async fn create(&self, row: &UsageRecordRow) -> Result<UsageRecordRow, StorageError> {
        let result = sqlx::query_as::<_, UsageRecordRow>(
            "INSERT INTO usage_records (id, timestamp, request_id, model, provider,
                                        prompt_tokens, completion_tokens, cache_read_tokens,
                                        cache_write_tokens, estimated_cost_usd, channel, strategy)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             RETURNING *",
        )
        .bind(row.id)
        .bind(row.timestamp)
        .bind(&row.request_id)
        .bind(&row.model)
        .bind(&row.provider)
        .bind(row.prompt_tokens)
        .bind(row.completion_tokens)
        .bind(row.cache_read_tokens)
        .bind(row.cache_write_tokens)
        .bind(row.estimated_cost_usd)
        .bind(&row.channel)
        .bind(&row.strategy)
        .fetch_one(&self.pool)
        .await?;
        Ok(result)
    }

    /// Aggregate usage by model within a date range.
    /// Returns (model, total_tokens, total_cost).
    pub async fn aggregate_by_model(
        &self,
        since: jiff::Timestamp,
    ) -> Result<Vec<(String, i64, f64)>, StorageError> {
        let rows: Vec<ModelAggregate> = sqlx::query_as(
            "SELECT model,
                    SUM(prompt_tokens + completion_tokens) AS total_tokens,
                    SUM(estimated_cost_usd) AS total_cost
             FROM usage_records
             WHERE timestamp >= ?1
             GROUP BY model
             ORDER BY total_cost DESC",
        )
        .bind(crate::sqlite_types::SqlTs::from(since))
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| {
                (
                    r.model,
                    r.total_tokens.unwrap_or(0),
                    r.total_cost.unwrap_or(0.0),
                )
            })
            .collect())
    }

    /// Aggregate usage by day within a date range.
    /// Returns (date_string, total_cost).
    pub async fn aggregate_by_day(
        &self,
        since: jiff::Timestamp,
    ) -> Result<Vec<(String, f64)>, StorageError> {
        let rows: Vec<DayAggregate> = sqlx::query_as(
            "SELECT strftime('%Y-%m-%d', timestamp) AS day,
                    SUM(estimated_cost_usd) AS total_cost
             FROM usage_records
             WHERE timestamp >= ?1
             GROUP BY day
             ORDER BY day ASC",
        )
        .bind(crate::sqlite_types::SqlTs::from(since))
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| (r.day.unwrap_or_default(), r.total_cost.unwrap_or(0.0)))
            .collect())
    }

    /// Get total estimated cost for the current calendar month (UTC).
    pub async fn total_cost_current_month(&self) -> Result<f64, StorageError> {
        let now = jiff::Timestamp::now();
        let today = now.to_zoned(jiff::tz::TimeZone::UTC).date();
        let month_start_date =
            jiff::civil::Date::new(today.year(), today.month(), 1).expect("valid date");
        let month_start = month_start_date
            .at(0, 0, 0, 0)
            .to_zoned(jiff::tz::TimeZone::UTC)
            .expect("UTC always valid")
            .timestamp();
        let (_, cost) = self.totals_since(month_start).await?;
        Ok(cost)
    }

    /// Get total tokens (prompt + completion) since a timestamp.
    pub async fn total_tokens_since(&self, since: jiff::Timestamp) -> Result<i64, StorageError> {
        Ok(sqlx::query_scalar::<_, i64>(
            "SELECT COALESCE(SUM(prompt_tokens + completion_tokens), 0)
             FROM usage_records WHERE timestamp >= ?1",
        )
        .bind(crate::sqlite_types::SqlTs::from(since))
        .fetch_one(&self.pool)
        .await?)
    }

    /// Get total request count and cost since a timestamp.
    pub async fn totals_since(&self, since: jiff::Timestamp) -> Result<(i64, f64), StorageError> {
        let row: TotalAggregate = sqlx::query_as(
            "SELECT COUNT(*) AS total_requests,
                    COALESCE(SUM(estimated_cost_usd), 0) AS total_cost
             FROM usage_records WHERE timestamp >= ?1",
        )
        .bind(crate::sqlite_types::SqlTs::from(since))
        .fetch_one(&self.pool)
        .await?;
        Ok((
            row.total_requests.unwrap_or(0),
            row.total_cost.unwrap_or(0.0),
        ))
    }
}

#[derive(sqlx::FromRow)]
struct ModelAggregate {
    model: String,
    total_tokens: Option<i64>,
    total_cost: Option<f64>,
}

#[derive(sqlx::FromRow)]
struct DayAggregate {
    day: Option<String>,
    total_cost: Option<f64>,
}

#[derive(sqlx::FromRow)]
struct TotalAggregate {
    total_requests: Option<i64>,
    total_cost: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rows::usage::UsageRecordRow;

    #[tokio::test]
    async fn total_tokens_since_aggregates_correctly() {
        let pool = crate::StoragePool::connect_in_memory().await.unwrap();
        let repo = UsageRepo::new(pool.inner().clone());

        let now = jiff::Timestamp::now();
        let old = now - jiff::SignedDuration::from_hours(2);
        let recent = now - jiff::SignedDuration::from_mins(30);

        // Insert an old record (before the "since" cutoff)
        repo.create(&UsageRecordRow {
            id: uuid::Uuid::new_v4(),
            timestamp: crate::sqlite_types::SqlTs::from(old),
            request_id: "req-old".to_string(),
            model: "gpt-4o".to_string(),
            provider: "openai".to_string(),
            prompt_tokens: 100,
            completion_tokens: 50,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            estimated_cost_usd: 0.01,
            channel: "telegram".to_string(),
            strategy: "DirectResponse".to_string(),
        })
        .await
        .unwrap();

        // Insert a recent record (after the "since" cutoff)
        repo.create(&UsageRecordRow {
            id: uuid::Uuid::new_v4(),
            timestamp: crate::sqlite_types::SqlTs::from(recent),
            request_id: "req-recent".to_string(),
            model: "gpt-4o".to_string(),
            provider: "openai".to_string(),
            prompt_tokens: 200,
            completion_tokens: 100,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            estimated_cost_usd: 0.02,
            channel: "telegram".to_string(),
            strategy: "ToolAssisted".to_string(),
        })
        .await
        .unwrap();

        // Query since 1 hour ago — should only include the recent record
        let since = now - jiff::SignedDuration::from_hours(1);
        let total = repo.total_tokens_since(since).await.unwrap();
        assert_eq!(total, 300); // 200 + 100

        // Query since 3 hours ago — should include both records
        let since_all = now - jiff::SignedDuration::from_hours(3);
        let total_all = repo.total_tokens_since(since_all).await.unwrap();
        assert_eq!(total_all, 450); // (100+50) + (200+100)

        // Query since the future — should return 0
        let future = now + jiff::SignedDuration::from_hours(1);
        let total_future = repo.total_tokens_since(future).await.unwrap();
        assert_eq!(total_future, 0);
    }
}
