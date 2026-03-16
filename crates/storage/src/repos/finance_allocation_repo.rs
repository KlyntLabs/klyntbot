//! Repository for the `finance_allocation_targets` table.

use sqlx::SqlitePool;
use uuid::Uuid;

use crate::error::StorageError;
use crate::rows::finance::FinanceAllocationTargetRow;

/// Repository for allocation targets — desired asset-class weights per portfolio.
#[derive(Debug, Clone)]
pub struct FinanceAllocationRepo {
    pool: SqlitePool,
}

impl FinanceAllocationRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Upsert an allocation target (INSERT OR REPLACE on the UNIQUE(portfolio_id, asset_class) constraint).
    /// Returns the inserted/replaced row.
    pub async fn add(
        &self,
        portfolio_id: &str,
        asset_class: &str,
        target_weight: &str,
        tolerance_band: &str,
    ) -> Result<FinanceAllocationTargetRow, StorageError> {
        let id = Uuid::new_v4().to_string();
        let row = sqlx::query_as::<_, FinanceAllocationTargetRow>(
            r#"
            INSERT OR REPLACE INTO finance_allocation_targets
                (id, portfolio_id, asset_class, target_weight, tolerance_band, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
            RETURNING *
            "#,
        )
        .bind(&id)
        .bind(portfolio_id)
        .bind(asset_class)
        .bind(target_weight)
        .bind(tolerance_band)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// List all allocation targets for a portfolio, ordered by asset class.
    pub async fn list_by_portfolio(
        &self,
        portfolio_id: &str,
    ) -> Result<Vec<FinanceAllocationTargetRow>, StorageError> {
        let rows = sqlx::query_as::<_, FinanceAllocationTargetRow>(
            "SELECT * FROM finance_allocation_targets WHERE portfolio_id = ? ORDER BY asset_class",
        )
        .bind(portfolio_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Update target weight and tolerance band for an existing allocation target.
    pub async fn update(
        &self,
        id: &str,
        target_weight: &str,
        tolerance_band: &str,
    ) -> Result<FinanceAllocationTargetRow, StorageError> {
        let row = sqlx::query_as::<_, FinanceAllocationTargetRow>(
            r#"
            UPDATE finance_allocation_targets
            SET target_weight = ?, tolerance_band = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
            WHERE id = ?
            RETURNING *
            "#,
        )
        .bind(target_weight)
        .bind(tolerance_band)
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| StorageError::NotFound(format!("finance_allocation_target {id}")))?;
        Ok(row)
    }

    /// Delete an allocation target by id.
    pub async fn delete(&self, id: &str) -> Result<(), StorageError> {
        sqlx::query("DELETE FROM finance_allocation_targets WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
