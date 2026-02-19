//! Repository for the `finance_transactions` table.

use chrono::NaiveDate;
use sqlx::PgPool;

use crate::error::StorageError;
use crate::rows::finance::{
    FinanceTransactionFilter, FinanceTransactionPatch, FinanceTransactionRow,
};

/// Repository for transaction CRUD, QueryBuilder filtering, transfer lookups, and aggregation.
#[derive(Debug, Clone)]
pub struct FinanceTransactionRepo {
    pool: PgPool,
}

impl FinanceTransactionRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // -----------------------------------------------------------------------
    // CRUD
    // -----------------------------------------------------------------------

    /// Insert a new transaction. Returns the inserted row.
    pub async fn add(
        &self,
        row: &FinanceTransactionRow,
    ) -> Result<FinanceTransactionRow, StorageError> {
        let inserted = sqlx::query_as::<_, FinanceTransactionRow>(
            r#"
            INSERT INTO finance_transactions (
                id, account_id, tx_type, amount, currency,
                category, subcategory, counterparty, notes,
                tx_date, transfer_id, is_recurring, recurring_rule,
                created_at, updated_at
            ) VALUES (
                $1, $2, $3, $4, $5,
                $6, $7, $8, $9,
                $10, $11, $12, $13,
                $14, $15
            )
            RETURNING *
            "#,
        )
        .bind(&row.id)
        .bind(&row.account_id)
        .bind(&row.tx_type)
        .bind(row.amount)
        .bind(&row.currency)
        .bind(&row.category)
        .bind(&row.subcategory)
        .bind(&row.counterparty)
        .bind(&row.notes)
        .bind(row.tx_date)
        .bind(&row.transfer_id)
        .bind(row.is_recurring)
        .bind(&row.recurring_rule)
        .bind(row.created_at)
        .bind(row.updated_at)
        .fetch_one(&self.pool)
        .await?;

        Ok(inserted)
    }

    /// Get a single transaction by id. Returns `None` if not found.
    pub async fn get(&self, id: &str) -> Result<Option<FinanceTransactionRow>, StorageError> {
        let row = sqlx::query_as::<_, FinanceTransactionRow>(
            "SELECT * FROM finance_transactions WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Update mutable fields on a transaction.
    /// Balance adjustment is the responsibility of the caller (tool layer).
    pub async fn update(
        &self,
        patch: &FinanceTransactionPatch,
    ) -> Result<FinanceTransactionRow, StorageError> {
        let row = sqlx::query_as::<_, FinanceTransactionRow>(
            r#"
            UPDATE finance_transactions SET
                amount       = COALESCE($2, amount),
                category     = CASE WHEN $3 THEN $4 ELSE category END,
                subcategory  = CASE WHEN $5 THEN $6 ELSE subcategory END,
                counterparty = CASE WHEN $7 THEN $8 ELSE counterparty END,
                notes        = CASE WHEN $9 THEN $10 ELSE notes END,
                tx_date      = COALESCE($11, tx_date),
                updated_at   = now()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(&patch.id)
        .bind(patch.amount)
        .bind(patch.category.is_some())
        .bind(patch.category.as_ref().and_then(|v| v.as_deref()))
        .bind(patch.subcategory.is_some())
        .bind(patch.subcategory.as_ref().and_then(|v| v.as_deref()))
        .bind(patch.counterparty.is_some())
        .bind(patch.counterparty.as_ref().and_then(|v| v.as_deref()))
        .bind(patch.notes.is_some())
        .bind(patch.notes.as_ref().and_then(|v| v.as_deref()))
        .bind(patch.tx_date)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| StorageError::NotFound(format!("finance_transaction {}", patch.id)))?;

        Ok(row)
    }

    /// Delete a transaction. Returns the deleted row (for balance reversal), or `None` if not found.
    pub async fn delete(&self, id: &str) -> Result<Option<FinanceTransactionRow>, StorageError> {
        let row = sqlx::query_as::<_, FinanceTransactionRow>(
            "DELETE FROM finance_transactions WHERE id = $1 RETURNING *",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    // -----------------------------------------------------------------------
    // Filtering (QueryBuilder)
    // -----------------------------------------------------------------------

    /// List transactions matching the given filter. Default limit is 20, max 100.
    pub async fn list(
        &self,
        filter: &FinanceTransactionFilter,
    ) -> Result<Vec<FinanceTransactionRow>, StorageError> {
        let limit = filter.limit.unwrap_or(20).min(100);

        let mut qb = sqlx::QueryBuilder::<sqlx::Postgres>::new(
            "SELECT * FROM finance_transactions WHERE TRUE",
        );

        if let Some(ref account_id) = filter.account_id {
            qb.push(" AND account_id = ");
            qb.push_bind(account_id);
        }

        if let Some(ref tx_type) = filter.tx_type {
            qb.push(" AND tx_type = ");
            qb.push_bind(tx_type);
        }

        if let Some(ref category) = filter.category {
            qb.push(" AND category = ");
            qb.push_bind(category);
        }

        if let Some(date_from) = filter.date_from {
            qb.push(" AND tx_date >= ");
            qb.push_bind(date_from);
        }

        if let Some(date_to) = filter.date_to {
            qb.push(" AND tx_date <= ");
            qb.push_bind(date_to);
        }

        if let Some(amount_min) = filter.amount_min {
            qb.push(" AND amount >= ");
            qb.push_bind(amount_min);
        }

        if let Some(amount_max) = filter.amount_max {
            qb.push(" AND amount <= ");
            qb.push_bind(amount_max);
        }

        if let Some(ref query) = filter.query {
            let escaped = query
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_");
            let pattern = format!("%{escaped}%");
            qb.push(" AND (notes ILIKE ");
            qb.push_bind(pattern.clone());
            qb.push(" OR counterparty ILIKE ");
            qb.push_bind(pattern.clone());
            qb.push(" OR category ILIKE ");
            qb.push_bind(pattern);
            qb.push(")");
        }

        qb.push(" ORDER BY tx_date DESC, created_at DESC LIMIT ");
        qb.push_bind(limit);

        let rows = qb
            .build_query_as::<FinanceTransactionRow>()
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    // -----------------------------------------------------------------------
    // Transfer Operations
    // -----------------------------------------------------------------------

    /// Get both sides of a transfer by `transfer_id`.
    pub async fn get_by_transfer_id(
        &self,
        transfer_id: &str,
    ) -> Result<Vec<FinanceTransactionRow>, StorageError> {
        let rows = sqlx::query_as::<_, FinanceTransactionRow>(
            "SELECT * FROM finance_transactions WHERE transfer_id = $1 ORDER BY tx_type",
        )
        .bind(transfer_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // -----------------------------------------------------------------------
    // Aggregation
    // -----------------------------------------------------------------------

    /// Sum transaction amounts grouped by category for a given date range and type.
    /// Returns `(category, total)` pairs, sorted by total descending.
    pub async fn sum_by_category(
        &self,
        date_from: NaiveDate,
        date_to: NaiveDate,
        tx_type: &str,
    ) -> Result<Vec<(String, i64)>, StorageError> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT COALESCE(category, 'uncategorized') AS cat, COALESCE(SUM(amount), 0)::BIGINT AS total
            FROM finance_transactions
            WHERE tx_type = $3
              AND tx_date >= $1
              AND tx_date <= $2
            GROUP BY cat
            ORDER BY total DESC
            "#,
        )
        .bind(date_from)
        .bind(date_to)
        .bind(tx_type)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Sum transaction amounts grouped by calendar month for the last `n_periods` months.
    /// Returns `(period_label, total)` pairs in descending order (most recent first).
    /// `period_type` is currently always `"month"`.
    pub async fn sum_by_period(
        &self,
        tx_type: &str,
        n_periods: i32,
        _period_type: &str,
    ) -> Result<Vec<(String, i64)>, StorageError> {
        // Calculate cutoff: start of (n_periods - 1) months ago.
        let rows: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT
                TO_CHAR(DATE_TRUNC('month', tx_date), 'YYYY-MM') AS period_label,
                COALESCE(SUM(amount), 0)::BIGINT AS total
            FROM finance_transactions
            WHERE tx_type = $1
              AND tx_date >= (DATE_TRUNC('month', CURRENT_DATE) - (($2::INT - 1) * INTERVAL '1 month'))::DATE
            GROUP BY period_label
            ORDER BY period_label DESC
            "#,
        )
        .bind(tx_type)
        .bind(n_periods)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Return the most frequent counterparty→category pairings for auto-categorisation.
    /// Returns `(counterparty, category, count)` triples, sorted by count descending.
    pub async fn category_history(
        &self,
        limit: i64,
    ) -> Result<Vec<(String, String, i64)>, StorageError> {
        let rows: Vec<(String, String, i64)> = sqlx::query_as(
            r#"
            SELECT counterparty, category, COUNT(*) AS cnt
            FROM finance_transactions
            WHERE counterparty IS NOT NULL AND category IS NOT NULL
            GROUP BY counterparty, category
            ORDER BY cnt DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}
