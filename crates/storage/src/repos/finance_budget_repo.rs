//! Repository for the `finance_budgets` table, including the budget-usage join query.

use crate::rows::finance::{BudgetUsageRow, FinanceBudgetPatch, FinanceBudgetRow};

crud_repo!(FinanceBudgetRepo, "finance_budgets", FinanceBudgetRow, "finance_budget");

impl FinanceBudgetRepo {
    // -----------------------------------------------------------------------
    // CRUD (add + update are hand-written)
    // -----------------------------------------------------------------------

    /// Insert a new budget. Returns the inserted row.
    pub async fn add(&self, row: &FinanceBudgetRow) -> Result<FinanceBudgetRow, crate::error::StorageError> {
        let inserted = sqlx::query_as::<_, FinanceBudgetRow>(
            r#"
            INSERT INTO finance_budgets (
                id, name, amount, currency, period, category,
                method, jar_type, start_date, end_date, is_active,
                alert_threshold, created_at, updated_at
            ) VALUES (
                ?, ?, ?, ?, ?, ?,
                ?, ?, ?, ?, ?,
                ?, ?, ?
            )
            RETURNING *
            "#,
        )
        .bind(&row.id)
        .bind(&row.name)
        .bind(row.amount)
        .bind(&row.currency)
        .bind(&row.period)
        .bind(&row.category)
        .bind(&row.method)
        .bind(&row.jar_type)
        .bind(row.start_date)
        .bind(row.end_date)
        .bind(row.is_active)
        .bind(row.alert_threshold)
        .bind(row.created_at)
        .bind(row.updated_at)
        .fetch_one(&self.pool)
        .await?;

        Ok(inserted)
    }

    /// Update mutable fields on a budget.
    pub async fn update(
        &self,
        patch: &FinanceBudgetPatch,
    ) -> Result<FinanceBudgetRow, crate::error::StorageError> {
        let row = sqlx::query_as::<_, FinanceBudgetRow>(
            r#"
            UPDATE finance_budgets SET
                name      = COALESCE(?, name),
                amount    = COALESCE(?, amount),
                category  = CASE WHEN ? THEN ? ELSE category END,
                is_active = COALESCE(?, is_active),
                updated_at = datetime('now')
            WHERE id = ?
            RETURNING *
            "#,
        )
        .bind(&patch.name)
        .bind(patch.amount)
        .bind(patch.category.is_some())
        .bind(patch.category.as_ref().and_then(|v| v.as_deref()))
        .bind(patch.is_active)
        .bind(&patch.id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| crate::error::StorageError::NotFound(format!("finance_budget {}", patch.id)))?;

        Ok(row)
    }

    // -----------------------------------------------------------------------
    // Listing
    // -----------------------------------------------------------------------

    /// Return all active budgets (`is_active = TRUE`), ordered by creation date.
    pub async fn list_active(&self) -> Result<Vec<FinanceBudgetRow>, crate::error::StorageError> {
        let rows = sqlx::query_as::<_, FinanceBudgetRow>(
            "SELECT * FROM finance_budgets WHERE is_active = TRUE ORDER BY created_at",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Get the active budget for a specific category. Returns `None` if no match.
    pub async fn get_by_category(
        &self,
        category: &str,
    ) -> Result<Option<FinanceBudgetRow>, crate::error::StorageError> {
        let row = sqlx::query_as::<_, FinanceBudgetRow>(
            "SELECT * FROM finance_budgets WHERE category = ? AND is_active = TRUE LIMIT 1",
        )
        .bind(category)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    // -----------------------------------------------------------------------
    // Budget Usage (SQL JOIN)
    // -----------------------------------------------------------------------

    /// Return the budget row for `budget_id` with the `spent` amount calculated
    /// by summing matching expense transactions in the budget's current period.
    pub async fn budget_usage(&self, budget_id: &str) -> Result<BudgetUsageRow, crate::error::StorageError> {
        let row = sqlx::query_as::<_, BudgetUsageRow>(
            r#"
            SELECT
                b.id,
                b.name,
                b.amount,
                b.currency,
                b.period,
                b.category,
                b.method,
                b.jar_type,
                b.start_date,
                b.end_date,
                b.is_active,
                b.alert_threshold,
                b.created_at,
                b.updated_at,
                COALESCE(SUM(ft.amount), 0) AS spent
            FROM finance_budgets b
            LEFT JOIN finance_transactions ft ON
                ft.tx_type = 'expense'
                AND (b.category IS NULL OR ft.category = b.category)
                AND ft.tx_date >= CASE
                    WHEN b.period = 'monthly' THEN date('now', 'start of month')
                    WHEN b.period = 'weekly'  THEN date('now', '-' || ((strftime('%w', 'now') + 6) % 7) || ' days')
                    WHEN b.period = 'yearly'  THEN date('now', 'start of year')
                    ELSE b.start_date
                END
                AND ft.tx_date <= CASE
                    WHEN b.period = 'monthly' THEN date('now', 'start of month', '+1 month', '-1 day')
                    WHEN b.period = 'weekly'  THEN date('now', '-' || ((strftime('%w', 'now') + 6) % 7) || ' days', '+6 days')
                    WHEN b.period = 'yearly'  THEN date('now', 'start of year', '+1 year', '-1 day')
                    ELSE COALESCE(b.end_date, date('now'))
                END
            WHERE b.id = ?
            GROUP BY b.id
            "#,
        )
        .bind(budget_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| crate::error::StorageError::NotFound(format!("finance_budget {budget_id}")))?;

        Ok(row)
    }

    /// Return `BudgetUsageRow` for every active budget, ordered by creation date.
    pub async fn all_budget_usage(&self) -> Result<Vec<BudgetUsageRow>, crate::error::StorageError> {
        let rows = sqlx::query_as::<_, BudgetUsageRow>(
            r#"
            SELECT
                b.id,
                b.name,
                b.amount,
                b.currency,
                b.period,
                b.category,
                b.method,
                b.jar_type,
                b.start_date,
                b.end_date,
                b.is_active,
                b.alert_threshold,
                b.created_at,
                b.updated_at,
                COALESCE(SUM(ft.amount), 0) AS spent
            FROM finance_budgets b
            LEFT JOIN finance_transactions ft ON
                ft.tx_type = 'expense'
                AND (b.category IS NULL OR ft.category = b.category)
                AND ft.tx_date >= CASE
                    WHEN b.period = 'monthly' THEN date('now', 'start of month')
                    WHEN b.period = 'weekly'  THEN date('now', '-' || ((strftime('%w', 'now') + 6) % 7) || ' days')
                    WHEN b.period = 'yearly'  THEN date('now', 'start of year')
                    ELSE b.start_date
                END
                AND ft.tx_date <= CASE
                    WHEN b.period = 'monthly' THEN date('now', 'start of month', '+1 month', '-1 day')
                    WHEN b.period = 'weekly'  THEN date('now', '-' || ((strftime('%w', 'now') + 6) % 7) || ' days', '+6 days')
                    WHEN b.period = 'yearly'  THEN date('now', 'start of year', '+1 year', '-1 day')
                    ELSE COALESCE(b.end_date, date('now'))
                END
            WHERE b.is_active = TRUE
            GROUP BY b.id
            ORDER BY b.created_at
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }
}
