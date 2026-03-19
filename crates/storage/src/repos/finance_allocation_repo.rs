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

    /// Upsert an allocation target on the UNIQUE(portfolio_id, asset_class) constraint.
    ///
    /// Uses ON CONFLICT DO UPDATE instead of INSERT OR REPLACE to preserve
    /// the existing row's `id` (and any FK references to it) on conflict.
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
            INSERT INTO finance_allocation_targets
                (id, portfolio_id, asset_class, target_weight, tolerance_band, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
            ON CONFLICT(portfolio_id, asset_class) DO UPDATE SET
                target_weight = excluded.target_weight,
                tolerance_band = excluded.tolerance_band,
                updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StoragePool;

    async fn setup() -> FinanceAllocationRepo {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        FinanceAllocationRepo::new(pool.inner().clone())
    }

    /// Insert a portfolio so FK constraints are satisfied.
    async fn insert_portfolio(pool: &SqlitePool, id: &str) {
        sqlx::query(
            "INSERT INTO finance_portfolios (id, name, currency) VALUES (?1, 'Test', 'USD')",
        )
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_upsert_preserves_id_on_conflict() {
        let repo = setup().await;
        insert_portfolio(&repo.pool, "port-1").await;

        // Insert initial allocation
        let first = repo.add("port-1", "equity", "0.60", "0.05").await.unwrap();
        let original_id = first.id.clone();
        assert_eq!(first.target_weight, "0.60");

        // Upsert same (portfolio_id, asset_class) with new weight
        let second = repo.add("port-1", "equity", "0.70", "0.10").await.unwrap();

        // ID must be preserved (not a new UUID)
        assert_eq!(second.id, original_id, "upsert must preserve existing row id");
        assert_eq!(second.target_weight, "0.70");
        assert_eq!(second.tolerance_band, "0.10");

        // Only one row should exist
        let all = repo.list_by_portfolio("port-1").await.unwrap();
        assert_eq!(all.len(), 1);
    }
}
