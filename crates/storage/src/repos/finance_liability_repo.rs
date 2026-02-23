//! Repository for the `finance_liabilities` table.

use sqlx::SqlitePool;

use crate::error::StorageError;
use crate::rows::finance::{FinanceLiabilityPatch, FinanceLiabilityRow};

/// Repository for liability CRUD, listing, and balance aggregation.
#[derive(Debug, Clone)]
pub struct FinanceLiabilityRepo {
    pool: SqlitePool,
}

impl FinanceLiabilityRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // -----------------------------------------------------------------------
    // CRUD
    // -----------------------------------------------------------------------

    /// Insert a new liability. Returns the inserted row.
    pub async fn add(
        &self,
        row: &FinanceLiabilityRow,
    ) -> Result<FinanceLiabilityRow, StorageError> {
        let inserted = sqlx::query_as::<_, FinanceLiabilityRow>(
            r#"
            INSERT INTO finance_liabilities (
                id, name, liability_type, principal, remaining,
                currency, interest_rate, monthly_payment,
                due_date, notes, created_at, updated_at
            ) VALUES (
                ?, ?, ?, ?, ?,
                ?, ?, ?,
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
        .fetch_one(&self.pool)
        .await?;

        Ok(inserted)
    }

    /// Get a single liability by id. Returns `None` if not found.
    pub async fn get(&self, id: &str) -> Result<Option<FinanceLiabilityRow>, StorageError> {
        let row = sqlx::query_as::<_, FinanceLiabilityRow>(
            "SELECT * FROM finance_liabilities WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Update mutable fields on a liability.
    pub async fn update(
        &self,
        patch: &FinanceLiabilityPatch,
    ) -> Result<FinanceLiabilityRow, StorageError> {
        let row = sqlx::query_as::<_, FinanceLiabilityRow>(
            r#"
            UPDATE finance_liabilities SET
                remaining       = COALESCE(?, remaining),
                monthly_payment = CASE WHEN ? THEN ? ELSE monthly_payment END,
                interest_rate   = CASE WHEN ? THEN ? ELSE interest_rate END,
                notes           = CASE WHEN ? THEN ? ELSE notes END,
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
        .bind(&patch.id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| StorageError::NotFound(format!("finance_liability {}", patch.id)))?;

        Ok(row)
    }

    /// Delete a liability. Returns `true` if the row existed and was deleted.
    pub async fn delete(&self, id: &str) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM finance_liabilities WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    // -----------------------------------------------------------------------
    // Listing
    // -----------------------------------------------------------------------

    /// List all liabilities, ordered by creation date.
    pub async fn list_all(&self) -> Result<Vec<FinanceLiabilityRow>, StorageError> {
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
    pub async fn total_remaining_by_currency(&self) -> Result<Vec<(String, i64)>, StorageError> {
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
}
