//! Repository for the `finance_liabilities` table.

use crate::rows::finance::{FinanceLiabilityPatch, FinanceLiabilityRow};

crud_repo!(
    FinanceLiabilityRepo,
    "finance_liabilities",
    FinanceLiabilityRow,
    "finance_liability"
);

impl FinanceLiabilityRepo {
    // -----------------------------------------------------------------------
    // CRUD (add + update are hand-written)
    // -----------------------------------------------------------------------

    /// Insert a new liability. Returns the inserted row.
    pub async fn add(
        &self,
        row: &FinanceLiabilityRow,
    ) -> Result<FinanceLiabilityRow, crate::error::StorageError> {
        let inserted = sqlx::query_as::<_, FinanceLiabilityRow>(
            r#"
            INSERT INTO finance_liabilities (
                id, name, liability_type, principal, remaining,
                currency, interest_rate, monthly_payment,
                due_date, notes, created_at, updated_at,
                base_principal, base_remaining, base_currency, exchange_rate
            ) VALUES (
                ?, ?, ?, ?, ?,
                ?, ?, ?,
                ?, ?, ?, ?,
                ?, ?, ?, ?
            )
            RETURNING *
            "#,
        )
        .bind(&row.id)
        .bind(&row.name)
        .bind(&row.liability_type)
        .bind(row.principal)
        .bind(row.remaining)
        .bind(&row.currency)
        .bind(row.interest_rate)
        .bind(row.monthly_payment)
        .bind(row.due_date)
        .bind(&row.notes)
        .bind(row.created_at)
        .bind(row.updated_at)
        .bind(row.base_principal)
        .bind(row.base_remaining)
        .bind(&row.base_currency)
        .bind(row.exchange_rate)
        .fetch_one(&self.pool)
        .await?;

        Ok(inserted)
    }

    /// Update mutable fields on a liability.
    pub async fn update(
        &self,
        patch: &FinanceLiabilityPatch,
    ) -> Result<FinanceLiabilityRow, crate::error::StorageError> {
        let row = sqlx::query_as::<_, FinanceLiabilityRow>(
            r#"
            UPDATE finance_liabilities SET
                remaining       = COALESCE(?, remaining),
                monthly_payment = CASE WHEN ? THEN ? ELSE monthly_payment END,
                interest_rate   = CASE WHEN ? THEN ? ELSE interest_rate END,
                notes           = CASE WHEN ? THEN ? ELSE notes END,
                base_principal  = COALESCE(?, base_principal),
                base_remaining  = COALESCE(?, base_remaining),
                base_currency   = COALESCE(?, base_currency),
                exchange_rate   = COALESCE(?, exchange_rate),
                updated_at      = datetime('now')
            WHERE id = ?
            RETURNING *
            "#,
        )
        .bind(patch.remaining)
        .bind(patch.monthly_payment.is_some())
        .bind(patch.monthly_payment.as_ref().and_then(|v| *v))
        .bind(patch.interest_rate.is_some())
        .bind(patch.interest_rate.as_ref().and_then(|v| *v))
        .bind(patch.notes.is_some())
        .bind(patch.notes.as_ref().and_then(|v| v.as_deref()))
        .bind(patch.base_principal)
        .bind(patch.base_remaining)
        .bind(patch.base_currency.as_deref())
        .bind(patch.exchange_rate)
        .bind(&patch.id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| {
            crate::error::StorageError::NotFound(format!("finance_liability {}", patch.id))
        })?;

        Ok(row)
    }

    // -----------------------------------------------------------------------
    // Listing
    // -----------------------------------------------------------------------

    /// List all liabilities, ordered by creation date.
    pub async fn list_all(&self) -> Result<Vec<FinanceLiabilityRow>, crate::error::StorageError> {
        let rows = sqlx::query_as::<_, FinanceLiabilityRow>(
            "SELECT * FROM finance_liabilities ORDER BY created_at",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // -----------------------------------------------------------------------
    // Aggregation
    // -----------------------------------------------------------------------

    /// Sum remaining balances of all liabilities, grouped by currency.
    /// Returns `(currency, total_remaining)` pairs.
    pub async fn total_remaining_by_currency(
        &self,
    ) -> Result<Vec<(String, i64)>, crate::error::StorageError> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT currency, COALESCE(SUM(remaining), 0) AS total
            FROM finance_liabilities
            GROUP BY currency
            ORDER BY currency
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Sum `base_remaining` of all liabilities whose `base_currency` matches.
    ///
    /// Returns a single consolidated total in the user's home currency.
    pub async fn total_base_remaining(
        &self,
        base_currency: &str,
    ) -> Result<i64, crate::error::StorageError> {
        let row = sqlx::query_as::<_, (i64,)>(
            "SELECT COALESCE(SUM(base_remaining), 0) FROM finance_liabilities WHERE base_currency = ?",
        )
        .bind(base_currency)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
    }
}
