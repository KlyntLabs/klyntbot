//! Repository for the `finance_transactions` table.

use jiff::civil::Date;

use crate::rows::finance::{
    FinanceTransactionFilter, FinanceTransactionPatch, FinanceTransactionRow,
};

// Uses @no_delete because delete returns Option<Row> instead of bool.
crud_repo!(@no_delete FinanceTransactionRepo, "finance_transactions", FinanceTransactionRow, "finance_transaction");

impl FinanceTransactionRepo {
    // -----------------------------------------------------------------------
    // CRUD (add, update, delete are hand-written)
    // -----------------------------------------------------------------------

    /// Insert a new transaction. Returns the inserted row.
    pub async fn add(
        &self,
        row: &FinanceTransactionRow,
    ) -> Result<FinanceTransactionRow, crate::error::StorageError> {
        let inserted = sqlx::query_as::<_, FinanceTransactionRow>(
            r#"
            INSERT INTO finance_transactions (
                id, account_id, tx_type, amount, currency,
                category, subcategory, counterparty, notes,
                tx_date, transfer_id, is_recurring, recurring_rule,
                created_at, updated_at,
                base_amount, base_currency, exchange_rate
            ) VALUES (
                ?, ?, ?, ?, ?,
                ?, ?, ?, ?,
                ?, ?, ?, ?,
                ?, ?,
                ?, ?, ?
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
        .bind(row.base_amount)
        .bind(&row.base_currency)
        .bind(row.exchange_rate)
        .fetch_one(&self.pool)
        .await?;

        Ok(inserted)
    }

    /// Update mutable fields on a transaction.
    /// Balance adjustment is the responsibility of the caller (tool layer).
    pub async fn update(
        &self,
        patch: &FinanceTransactionPatch,
    ) -> Result<FinanceTransactionRow, crate::error::StorageError> {
        let row = sqlx::query_as::<_, FinanceTransactionRow>(
            r#"
            UPDATE finance_transactions SET
                amount        = COALESCE(?, amount),
                category      = CASE WHEN ? THEN ? ELSE category END,
                subcategory   = CASE WHEN ? THEN ? ELSE subcategory END,
                counterparty  = CASE WHEN ? THEN ? ELSE counterparty END,
                notes         = CASE WHEN ? THEN ? ELSE notes END,
                tx_date       = COALESCE(?, tx_date),
                base_amount   = COALESCE(?, base_amount),
                base_currency = COALESCE(?, base_currency),
                exchange_rate = COALESCE(?, exchange_rate),
                updated_at    = (unixepoch('now') * 1000)
            WHERE id = ?
            RETURNING *
            "#,
        )
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
        .bind(patch.base_amount)
        .bind(patch.base_currency.as_deref())
        .bind(patch.exchange_rate)
        .bind(&patch.id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| {
            crate::error::StorageError::NotFound(format!("finance_transaction {}", patch.id))
        })?;

        Ok(row)
    }

    /// Delete a transaction. Returns the deleted row (for balance reversal), or `None` if not found.
    pub async fn delete(
        &self,
        id: &str,
    ) -> Result<Option<FinanceTransactionRow>, crate::error::StorageError> {
        let row = sqlx::query_as::<_, FinanceTransactionRow>(
            "DELETE FROM finance_transactions WHERE id = ? RETURNING *",
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
    ) -> Result<Vec<FinanceTransactionRow>, crate::error::StorageError> {
        let limit = filter.limit.unwrap_or(20).min(100);

        let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
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
            let pattern = format!("%{}%", crate::macros::escape_like(query));
            qb.push(" AND (notes LIKE ");
            qb.push_bind(pattern.clone());
            qb.push(" ESCAPE '\\' OR counterparty LIKE ");
            qb.push_bind(pattern.clone());
            qb.push(" ESCAPE '\\' OR category LIKE ");
            qb.push_bind(pattern);
            qb.push(" ESCAPE '\\')");
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
    ) -> Result<Vec<FinanceTransactionRow>, crate::error::StorageError> {
        let rows = sqlx::query_as::<_, FinanceTransactionRow>(
            "SELECT * FROM finance_transactions WHERE transfer_id = ? ORDER BY tx_type",
        )
        .bind(transfer_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // -----------------------------------------------------------------------
    // Aggregation
    // -----------------------------------------------------------------------

    /// Sum transaction base amounts grouped by category for a given date range and type.
    /// Returns `(category, total)` pairs in the user's base currency, sorted by total descending.
    pub async fn sum_by_category(
        &self,
        date_from: Date,
        date_to: Date,
        tx_type: &str,
        base_currency: &str,
    ) -> Result<Vec<(String, i64)>, crate::error::StorageError> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT COALESCE(category, 'uncategorized') AS cat, COALESCE(SUM(base_amount), 0) AS total
            FROM finance_transactions
            WHERE tx_date >= ?
              AND tx_date <= ?
              AND tx_type = ?
              AND base_currency = ?
              AND transfer_id IS NULL
            GROUP BY cat
            ORDER BY total DESC
            "#,
        )
        .bind(date_from.to_string())
        .bind(date_to.to_string())
        .bind(tx_type)
        .bind(base_currency)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Sum transaction base amounts grouped by calendar month for the last `n_periods` months.
    /// Returns `(period_label, total)` pairs in the user's base currency, in descending order
    /// (most recent first). `period_type` is currently always `"month"`.
    pub async fn sum_by_period(
        &self,
        tx_type: &str,
        n_periods: i32,
        base_currency: &str,
    ) -> Result<Vec<(String, i64)>, crate::error::StorageError> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT
                strftime('%Y-%m', tx_date) AS period_label,
                COALESCE(SUM(base_amount), 0)   AS total
            FROM finance_transactions
            WHERE tx_type = ?
              AND base_currency = ?
              AND tx_date >= date('now', 'localtime', 'start of month', '-' || (? - 1) || ' months')
              AND transfer_id IS NULL
            GROUP BY period_label
            ORDER BY period_label DESC
            "#,
        )
        .bind(tx_type)
        .bind(base_currency)
        .bind(n_periods)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Sum daily spending (expenses) for a date range in the user's base currency.
    /// Returns `(date_str, total_spending, tx_count)` triples ordered by date ascending.
    pub async fn daily_spending(
        &self,
        date_from: Date,
        date_to: Date,
        base_currency: &str,
    ) -> Result<Vec<(String, i64, i32)>, crate::error::StorageError> {
        let rows: Vec<(String, i64, i32)> = sqlx::query_as(
            r#"
            SELECT
                tx_date AS date,
                COALESCE(SUM(base_amount), 0) AS total_spending,
                CAST(COUNT(*) AS INTEGER) AS tx_count
            FROM finance_transactions
            WHERE tx_type = 'expense'
              AND tx_date >= ?
              AND tx_date <= ?
              AND base_currency = ?
              AND transfer_id IS NULL
            GROUP BY tx_date
            ORDER BY tx_date
            "#,
        )
        .bind(date_from.to_string())
        .bind(date_to.to_string())
        .bind(base_currency)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Sum transaction base amounts for a given type and date range in the user's base currency.
    /// Returns the total or 0 if no matching transactions exist.
    pub async fn sum_by_type_in_range(
        &self,
        tx_type: &str,
        date_from: Date,
        date_to: Date,
        base_currency: &str,
    ) -> Result<i64, crate::error::StorageError> {
        let (total,): (i64,) = sqlx::query_as(
            r#"
            SELECT COALESCE(SUM(base_amount), 0)
            FROM finance_transactions
            WHERE tx_type = ?
              AND tx_date >= ?
              AND tx_date <= ?
              AND base_currency = ?
              AND transfer_id IS NULL
            "#,
        )
        .bind(tx_type)
        .bind(date_from.to_string())
        .bind(date_to.to_string())
        .bind(base_currency)
        .fetch_one(&self.pool)
        .await?;
        Ok(total)
    }

    /// Return the most frequent counterparty→category pairings for auto-categorisation.
    /// Returns `(counterparty, category, count)` triples, sorted by count descending.
    pub async fn category_history(
        &self,
        limit: i64,
    ) -> Result<Vec<(String, String, i64)>, crate::error::StorageError> {
        let rows: Vec<(String, String, i64)> = sqlx::query_as(
            r#"
            SELECT counterparty, category, COUNT(*) AS cnt
            FROM finance_transactions
            WHERE counterparty IS NOT NULL AND category IS NOT NULL
            GROUP BY counterparty, category
            ORDER BY cnt DESC
            LIMIT ?
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}
