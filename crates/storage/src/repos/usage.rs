//! Usage repository — usage_records table.

use chrono::{Datelike, DateTime, NaiveTime, Utc};
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
        since: DateTime<Utc>,
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
        .bind(since)
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
        since: DateTime<Utc>,
    ) -> Result<Vec<(String, f64)>, StorageError> {
        let rows: Vec<DayAggregate> = sqlx::query_as(
            "SELECT strftime('%Y-%m-%d', timestamp) AS day,
                    SUM(estimated_cost_usd) AS total_cost
             FROM usage_records
             WHERE timestamp >= ?1
             GROUP BY day
             ORDER BY day ASC",
        )
        .bind(since)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| (r.day.unwrap_or_default(), r.total_cost.unwrap_or(0.0)))
            .collect())
    }

    /// Get total estimated cost for the current calendar month (UTC).
    pub async fn total_cost_current_month(&self) -> Result<f64, StorageError> {
        let now = Utc::now();
        let first_of_month = now.date_naive().with_day(1).unwrap_or(now.date_naive());
        let month_start =
            first_of_month.and_time(NaiveTime::MIN);
        let month_start_utc = DateTime::<Utc>::from_naive_utc_and_offset(month_start, Utc);
        let (_, cost) = self.totals_since(month_start_utc).await?;
        Ok(cost)
    }

    /// Get total request count and cost since a timestamp.
    pub async fn totals_since(&self, since: DateTime<Utc>) -> Result<(i64, f64), StorageError> {
        let row: TotalAggregate = sqlx::query_as(
            "SELECT COUNT(*) AS total_requests,
                    COALESCE(SUM(estimated_cost_usd), 0) AS total_cost
             FROM usage_records WHERE timestamp >= ?1",
        )
        .bind(since)
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
