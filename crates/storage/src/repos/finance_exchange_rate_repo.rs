//! Repository for the `finance_exchange_rates` table (composite PK, no `id` column).

use chrono::Utc;
use sqlx::SqlitePool;

use crate::error::StorageError;
use crate::rows::finance::FinanceExchangeRateRow;

/// Cache repository for exchange rates. Manual struct (not `crud_repo!`) because
/// this table has a composite primary key `(from_currency, to_currency)`.
#[derive(Debug, Clone)]
pub struct FinanceExchangeRateRepo {
    pool: SqlitePool,
}

impl FinanceExchangeRateRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert or update a single exchange rate.
    pub async fn upsert(&self, from: &str, to: &str, rate: f64) -> Result<(), StorageError> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO finance_exchange_rates (from_currency, to_currency, rate, fetched_at)
            VALUES (?, ?, ?, ?)
            ON CONFLICT (from_currency, to_currency) DO UPDATE SET
                rate = excluded.rate,
                fetched_at = excluded.fetched_at
            "#,
        )
        .bind(from)
        .bind(to)
        .bind(rate)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get a rate only if it was fetched within `max_age_minutes`. Excludes sentinel rows.
    pub async fn get_fresh(
        &self,
        from: &str,
        to: &str,
        max_age_minutes: i64,
    ) -> Result<Option<FinanceExchangeRateRow>, StorageError> {
        let row = sqlx::query_as::<_, FinanceExchangeRateRow>(
            r#"
            SELECT from_currency, to_currency, rate, fetched_at
            FROM finance_exchange_rates
            WHERE from_currency = ? AND to_currency = ?
              AND from_currency NOT LIKE '__%'
              AND fetched_at >= datetime('now', '-' || ? || ' minutes')
            "#,
        )
        .bind(from)
        .bind(to)
        .bind(max_age_minutes)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Get a rate regardless of age (but still exclude sentinel rows).
    pub async fn get_stale(
        &self,
        from: &str,
        to: &str,
    ) -> Result<Option<FinanceExchangeRateRow>, StorageError> {
        let row = sqlx::query_as::<_, FinanceExchangeRateRow>(
            r#"
            SELECT from_currency, to_currency, rate, fetched_at
            FROM finance_exchange_rates
            WHERE from_currency = ? AND to_currency = ?
              AND from_currency NOT LIKE '__%'
            "#,
        )
        .bind(from)
        .bind(to)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Upsert a batch of rates from a single base currency.
    /// `rates` is a slice of `(target_currency, rate)` pairs.
    pub async fn upsert_batch(
        &self,
        base: &str,
        rates: &[(&str, f64)],
    ) -> Result<(), StorageError> {
        let now = Utc::now().to_rfc3339();
        for (target, rate) in rates {
            sqlx::query(
                r#"
                INSERT INTO finance_exchange_rates (from_currency, to_currency, rate, fetched_at)
                VALUES (?, ?, ?, ?)
                ON CONFLICT (from_currency, to_currency) DO UPDATE SET
                    rate = excluded.rate,
                    fetched_at = excluded.fetched_at
                "#,
            )
            .bind(base)
            .bind(target)
            .bind(rate)
            .bind(&now)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    /// Get a sentinel row (from_currency starts with `__`).
    pub async fn get_sentinel(
        &self,
        key: &str,
    ) -> Result<Option<FinanceExchangeRateRow>, StorageError> {
        let row = sqlx::query_as::<_, FinanceExchangeRateRow>(
            r#"
            SELECT from_currency, to_currency, rate, fetched_at
            FROM finance_exchange_rates
            WHERE from_currency = ?
            "#,
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Set a sentinel row (for tracking metadata like last-fetch timestamps).
    pub async fn set_sentinel(&self, from: &str, to: &str, rate: f64) -> Result<(), StorageError> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO finance_exchange_rates (from_currency, to_currency, rate, fetched_at)
            VALUES (?, ?, ?, ?)
            ON CONFLICT (from_currency, to_currency) DO UPDATE SET
                rate = excluded.rate,
                fetched_at = excluded.fetched_at
            "#,
        )
        .bind(from)
        .bind(to)
        .bind(rate)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Delete all sentinel rows for a given `from_currency` key.
    pub async fn delete_sentinel(&self, from: &str) -> Result<(), StorageError> {
        sqlx::query("DELETE FROM finance_exchange_rates WHERE from_currency = ?")
            .bind(from)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
