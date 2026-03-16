//! Repository for the `finance_net_worth_snapshots` table.

use sqlx::SqlitePool;
use uuid::Uuid;

use crate::error::StorageError;
use crate::rows::finance::FinanceNetWorthSnapshotRow;

/// Repository for net-worth snapshots — periodic point-in-time captures.
#[derive(Debug, Clone)]
pub struct FinanceSnapshotRepo {
    pool: SqlitePool,
}

impl FinanceSnapshotRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert a new net-worth snapshot. Returns the inserted row.
    #[allow(clippy::too_many_arguments)]
    pub async fn add(
        &self,
        snapshot_date: &str,
        currency: &str,
        accounts_total: i64,
        investments_total: i64,
        liabilities_total: i64,
        net_worth: i64,
        breakdown_json: &str,
    ) -> Result<FinanceNetWorthSnapshotRow, StorageError> {
        let id = Uuid::new_v4().to_string();
        let row = sqlx::query_as::<_, FinanceNetWorthSnapshotRow>(
            r#"
            INSERT INTO finance_net_worth_snapshots
                (id, snapshot_date, currency, accounts_total, investments_total,
                 liabilities_total, net_worth, breakdown, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
            RETURNING *
            "#,
        )
        .bind(&id)
        .bind(snapshot_date)
        .bind(currency)
        .bind(accounts_total)
        .bind(investments_total)
        .bind(liabilities_total)
        .bind(net_worth)
        .bind(breakdown_json)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// List snapshots within a date range for a given currency, ordered by date ascending.
    pub async fn list_by_date_range(
        &self,
        start_date: &str,
        end_date: &str,
        currency: &str,
    ) -> Result<Vec<FinanceNetWorthSnapshotRow>, StorageError> {
        let rows = sqlx::query_as::<_, FinanceNetWorthSnapshotRow>(
            r#"
            SELECT * FROM finance_net_worth_snapshots
            WHERE snapshot_date >= ? AND snapshot_date <= ? AND currency = ?
            ORDER BY snapshot_date
            "#,
        )
        .bind(start_date)
        .bind(end_date)
        .bind(currency)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}
