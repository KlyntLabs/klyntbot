//! Repository for the `finance_budgets` table, including the budget-usage join query.

use sqlx::PgPool;

use crate::error::StorageError;
use crate::rows::finance::{BudgetUsageRow, FinanceBudgetPatch, FinanceBudgetRow};

/// Repository for budget CRUD, active-budget listing, and usage aggregation.
#[derive(Debug, Clone)]
pub struct FinanceBudgetRepo {
    pool: PgPool,
}

impl FinanceBudgetRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // -----------------------------------------------------------------------
    // CRUD
    // -----------------------------------------------------------------------

    /// Insert a new budget. Returns the inserted row.
    pub async fn add(&self, row: &FinanceBudgetRow) -> Result<FinanceBudgetRow, StorageError> {
        let inserted = sqlx::query_as::<_, FinanceBudgetRow>(
            r#"
            INSERT INTO finance_budgets (
                id, name, amount, currency, period, category,
                method, jar_type, start_date, end_date, is_active,
                alert_threshold, created_at, updated_at
            ) VALUES (
                $1, $2, $3, $4, $5, $6,
                $7, $8, $9, $10, $11,
                $12, $13, $14
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

    /// Get a single budget by id. Returns `None` if not found.
    pub async fn get(&self, id: &str) -> Result<Option<FinanceBudgetRow>, StorageError> {
        let row =
            sqlx::query_as::<_, FinanceBudgetRow>("SELECT * FROM finance_budgets WHERE id = $1")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row)
    }

    /// Update mutable fields on a budget.
    pub async fn update(
        &self,
        patch: &FinanceBudgetPatch,
    ) -> Result<FinanceBudgetRow, StorageError> {
        let row = sqlx::query_as::<_, FinanceBudgetRow>(
            r#"
            UPDATE finance_budgets SET
                name      = COALESCE($2, name),
                amount    = COALESCE($3, amount),
                category  = CASE WHEN $4 THEN $5 ELSE category END,
                is_active = COALESCE($6, is_active),
                updated_at = now()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(&patch.id)
        .bind(&patch.name)
        .bind(patch.amount)
        .bind(patch.category.is_some())
        .bind(patch.category.as_ref().and_then(|v| v.as_deref()))
        .bind(patch.is_active)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| StorageError::NotFound(format!("finance_budget {}", patch.id)))?;

        Ok(row)
    }

    /// Delete a budget. Returns `true` if the row existed and was deleted.
    pub async fn delete(&self, id: &str) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM finance_budgets WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    // -----------------------------------------------------------------------
    // Listing
    // -----------------------------------------------------------------------

    /// Return all active budgets (`is_active = TRUE`), ordered by creation date.
    pub async fn list_active(&self) -> Result<Vec<FinanceBudgetRow>, StorageError> {
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
    ) -> Result<Option<FinanceBudgetRow>, StorageError> {
        let row = sqlx::query_as::<_, FinanceBudgetRow>(
            "SELECT * FROM finance_budgets WHERE category = $1 AND is_active = TRUE LIMIT 1",
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
    ///
    /// Period boundaries are derived from `b.period` + `CURRENT_DATE`:
    /// - `monthly` → first to last day of the current calendar month
    /// - `weekly`  → Monday to Sunday of the current ISO week
    /// - `yearly`  → first to last day of the current calendar year
    /// - `custom`  → `b.start_date` to `b.end_date` (or today if no end_date)
    ///
    /// If `b.category IS NULL` the budget is a total budget and all expense
    /// categories are summed.
    ///
    /// Returns `StorageError::NotFound` when `budget_id` does not exist.
    pub async fn budget_usage(&self, budget_id: &str) -> Result<BudgetUsageRow, StorageError> {
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
                    WHEN b.period = 'monthly' THEN DATE_TRUNC('month', CURRENT_DATE)::DATE
                    WHEN b.period = 'weekly'  THEN DATE_TRUNC('week', CURRENT_DATE)::DATE
                    WHEN b.period = 'yearly'  THEN DATE_TRUNC('year', CURRENT_DATE)::DATE
                    ELSE b.start_date
                END
                AND ft.tx_date <= CASE
                    WHEN b.period = 'monthly' THEN (DATE_TRUNC('month', CURRENT_DATE) + INTERVAL '1 month' - INTERVAL '1 day')::DATE
                    WHEN b.period = 'weekly'  THEN (DATE_TRUNC('week', CURRENT_DATE) + INTERVAL '6 days')::DATE
                    WHEN b.period = 'yearly'  THEN (DATE_TRUNC('year', CURRENT_DATE) + INTERVAL '1 year' - INTERVAL '1 day')::DATE
                    ELSE COALESCE(b.end_date, CURRENT_DATE)
                END
            WHERE b.id = $1
            GROUP BY b.id
            "#,
        )
        .bind(budget_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| StorageError::NotFound(format!("finance_budget {budget_id}")))?;

        Ok(row)
    }

    /// Return `BudgetUsageRow` for every active budget, ordered by creation date.
    pub async fn all_budget_usage(&self) -> Result<Vec<BudgetUsageRow>, StorageError> {
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
                    WHEN b.period = 'monthly' THEN DATE_TRUNC('month', CURRENT_DATE)::DATE
                    WHEN b.period = 'weekly'  THEN DATE_TRUNC('week', CURRENT_DATE)::DATE
                    WHEN b.period = 'yearly'  THEN DATE_TRUNC('year', CURRENT_DATE)::DATE
                    ELSE b.start_date
                END
                AND ft.tx_date <= CASE
                    WHEN b.period = 'monthly' THEN (DATE_TRUNC('month', CURRENT_DATE) + INTERVAL '1 month' - INTERVAL '1 day')::DATE
                    WHEN b.period = 'weekly'  THEN (DATE_TRUNC('week', CURRENT_DATE) + INTERVAL '6 days')::DATE
                    WHEN b.period = 'yearly'  THEN (DATE_TRUNC('year', CURRENT_DATE) + INTERVAL '1 year' - INTERVAL '1 day')::DATE
                    ELSE COALESCE(b.end_date, CURRENT_DATE)
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
