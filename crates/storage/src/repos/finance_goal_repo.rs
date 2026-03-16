//! Repository for the `finance_goals` table.

use crate::rows::finance::{FinanceGoalPatch, FinanceGoalRow};

crud_repo!(
    FinanceGoalRepo,
    "finance_goals",
    FinanceGoalRow,
    "finance_goal"
);

impl FinanceGoalRepo {
    // -----------------------------------------------------------------------
    // CRUD (add + update are hand-written)
    // -----------------------------------------------------------------------

    /// Insert a new goal. Returns the inserted row.
    pub async fn add(
        &self,
        row: &FinanceGoalRow,
    ) -> Result<FinanceGoalRow, crate::error::StorageError> {
        let inserted = sqlx::query_as::<_, FinanceGoalRow>(
            r#"
            INSERT INTO finance_goals (
                id, name, goal_type, target_amount, current_amount,
                currency, status, deadline, monthly_contribution,
                expected_return_rate, inflation_rate, notes,
                created_at, updated_at,
                base_target_amount, base_current_amount, base_currency, exchange_rate
            ) VALUES (
                ?, ?, ?, ?, ?,
                ?, ?, ?, ?,
                ?, ?, ?,
                ?, ?,
                ?, ?, ?, ?
            )
            RETURNING *
            "#,
        )
        .bind(&row.id)
        .bind(&row.name)
        .bind(&row.goal_type)
        .bind(row.target_amount)
        .bind(row.current_amount)
        .bind(&row.currency)
        .bind(&row.status)
        .bind(row.deadline)
        .bind(row.monthly_contribution)
        .bind(row.expected_return_rate)
        .bind(row.inflation_rate)
        .bind(&row.notes)
        .bind(row.created_at)
        .bind(row.updated_at)
        .bind(row.base_target_amount)
        .bind(row.base_current_amount)
        .bind(&row.base_currency)
        .bind(row.exchange_rate)
        .fetch_one(&self.pool)
        .await?;

        Ok(inserted)
    }

    /// Update mutable fields on a goal.
    pub async fn update(
        &self,
        patch: &FinanceGoalPatch,
    ) -> Result<FinanceGoalRow, crate::error::StorageError> {
        let row = sqlx::query_as::<_, FinanceGoalRow>(
            r#"
            UPDATE finance_goals SET
                name                 = COALESCE(?, name),
                current_amount       = COALESCE(?, current_amount),
                target_amount        = COALESCE(?, target_amount),
                monthly_contribution = CASE WHEN ? THEN ? ELSE monthly_contribution END,
                expected_return_rate = CASE WHEN ? THEN ? ELSE expected_return_rate END,
                inflation_rate       = CASE WHEN ? THEN ? ELSE inflation_rate END,
                deadline             = CASE WHEN ? THEN ? ELSE deadline END,
                status               = COALESCE(?, status),
                base_target_amount   = COALESCE(?, base_target_amount),
                base_current_amount  = COALESCE(?, base_current_amount),
                base_currency        = COALESCE(?, base_currency),
                exchange_rate        = COALESCE(?, exchange_rate),
                updated_at           = datetime('now')
            WHERE id = ?
            RETURNING *
            "#,
        )
        .bind(&patch.name)
        .bind(patch.current_amount)
        .bind(patch.target_amount)
        .bind(patch.monthly_contribution.is_some())
        .bind(patch.monthly_contribution.as_ref().and_then(|v| *v))
        .bind(patch.expected_return_rate.is_some())
        .bind(patch.expected_return_rate.as_ref().and_then(|v| *v))
        .bind(patch.inflation_rate.is_some())
        .bind(patch.inflation_rate.as_ref().and_then(|v| *v))
        .bind(patch.deadline.is_some())
        .bind(patch.deadline.as_ref().and_then(|v| *v))
        .bind(&patch.status)
        .bind(patch.base_target_amount)
        .bind(patch.base_current_amount)
        .bind(patch.base_currency.as_deref())
        .bind(patch.exchange_rate)
        .bind(&patch.id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| {
            crate::error::StorageError::NotFound(format!("finance_goal {}", patch.id))
        })?;

        Ok(row)
    }

    // -----------------------------------------------------------------------
    // Listing
    // -----------------------------------------------------------------------

    /// Return goals with `status = 'active'`, ordered by creation date.
    pub async fn list_active(&self) -> Result<Vec<FinanceGoalRow>, crate::error::StorageError> {
        let rows = sqlx::query_as::<_, FinanceGoalRow>(
            "SELECT * FROM finance_goals WHERE status = 'active' ORDER BY created_at",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Return all goals regardless of status, ordered by creation date.
    pub async fn list_all(&self) -> Result<Vec<FinanceGoalRow>, crate::error::StorageError> {
        let rows =
            sqlx::query_as::<_, FinanceGoalRow>("SELECT * FROM finance_goals ORDER BY created_at")
                .fetch_all(&self.pool)
                .await?;
        Ok(rows)
    }

    // -----------------------------------------------------------------------
    // Progress
    // -----------------------------------------------------------------------

    /// Set `current_amount` directly on a goal. Returns the updated row.
    pub async fn update_progress(
        &self,
        id: &str,
        current_amount: i64,
    ) -> Result<FinanceGoalRow, crate::error::StorageError> {
        let row = sqlx::query_as::<_, FinanceGoalRow>(
            r#"
            UPDATE finance_goals
            SET current_amount = ?, updated_at = datetime('now')
            WHERE id = ?
            RETURNING *
            "#,
        )
        .bind(current_amount)
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| crate::error::StorageError::NotFound(format!("finance_goal {id}")))?;
        Ok(row)
    }
}
