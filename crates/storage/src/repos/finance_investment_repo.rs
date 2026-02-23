//! Repository for `finance_portfolios`, `finance_investments`, and
//! `finance_investment_transactions` — three related tables managed together.

use sqlx::SqlitePool;

use crate::error::StorageError;
use crate::rows::finance::{
    FinanceInvestmentFilter, FinanceInvestmentPatch, FinanceInvestmentRow, FinanceInvestmentTxRow,
    FinancePortfolioRow, PortfolioSummaryRow,
};

/// Repository covering portfolios, investments, and investment transactions.
#[derive(Debug, Clone)]
pub struct FinanceInvestmentRepo {
    pool: SqlitePool,
}

impl FinanceInvestmentRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // -----------------------------------------------------------------------
    // Portfolio CRUD
    // -----------------------------------------------------------------------

    /// Insert a new portfolio. Returns the inserted row.
    pub async fn add_portfolio(
        &self,
        row: &FinancePortfolioRow,
    ) -> Result<FinancePortfolioRow, StorageError> {
        let inserted = sqlx::query_as::<_, FinancePortfolioRow>(
            r#"
            INSERT INTO finance_portfolios (id, name, description, currency, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?)
            RETURNING *
            "#,
        )
        .bind(&row.id)
        .bind(&row.name)
        .bind(&row.description)
        .bind(&row.currency)
        .bind(row.created_at)
        .bind(row.updated_at)
        .fetch_one(&self.pool)
        .await?;
        Ok(inserted)
    }

    /// Get a single portfolio by id. Returns `None` if not found.
    pub async fn get_portfolio(
        &self,
        id: &str,
    ) -> Result<Option<FinancePortfolioRow>, StorageError> {
        let row = sqlx::query_as::<_, FinancePortfolioRow>(
            "SELECT * FROM finance_portfolios WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// List all portfolios, ordered by creation date.
    pub async fn list_portfolios(&self) -> Result<Vec<FinancePortfolioRow>, StorageError> {
        let rows = sqlx::query_as::<_, FinancePortfolioRow>(
            "SELECT * FROM finance_portfolios ORDER BY created_at",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Delete a portfolio and cascade-delete all its investments and their transactions.
    pub async fn delete_portfolio(&self, id: &str) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM finance_portfolios WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    // -----------------------------------------------------------------------
    // Investment CRUD
    // -----------------------------------------------------------------------

    /// Insert a new investment holding. Returns the inserted row.
    pub async fn add_investment(
        &self,
        row: &FinanceInvestmentRow,
    ) -> Result<FinanceInvestmentRow, StorageError> {
        let inserted = sqlx::query_as::<_, FinanceInvestmentRow>(
            r#"
            INSERT INTO finance_investments (
                id, portfolio_id, asset_type, symbol, name, quantity,
                cost_basis, currency, current_price, current_value,
                purchase_date, notes, created_at, updated_at
            ) VALUES (
                ?, ?, ?, ?, ?, ?,
                ?, ?, ?, ?,
                ?, ?, ?, ?
            )
            RETURNING *
            "#,
        )
        .bind(&row.id)
        .bind(&row.portfolio_id)
        .bind(&row.asset_type)
        .bind(&row.symbol)
        .bind(&row.name)
        .bind(row.quantity)
        .bind(row.cost_basis)
        .bind(&row.currency)
        .bind(row.current_price)
        .bind(row.current_value)
        .bind(row.purchase_date)
        .bind(&row.notes)
        .bind(row.created_at)
        .bind(row.updated_at)
        .fetch_one(&self.pool)
        .await?;
        Ok(inserted)
    }

    /// Get a single investment by id. Returns `None` if not found.
    pub async fn get_investment(
        &self,
        id: &str,
    ) -> Result<Option<FinanceInvestmentRow>, StorageError> {
        let row = sqlx::query_as::<_, FinanceInvestmentRow>(
            "SELECT * FROM finance_investments WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Update mutable fields on an investment.
    pub async fn update_investment(
        &self,
        patch: &FinanceInvestmentPatch,
    ) -> Result<FinanceInvestmentRow, StorageError> {
        let row = sqlx::query_as::<_, FinanceInvestmentRow>(
            r#"
            UPDATE finance_investments SET
                current_price = CASE WHEN ? THEN ? ELSE current_price END,
                current_value = CASE WHEN ? THEN ? ELSE current_value END,
                quantity      = COALESCE(?, quantity),
                cost_basis    = COALESCE(?, cost_basis),
                notes         = CASE WHEN ? THEN ? ELSE notes END,
                updated_at    = datetime('now')
            WHERE id = ?
            RETURNING *
            "#,
        )
        .bind(patch.current_price.is_some())
        .bind(patch.current_price.as_ref().and_then(|v| *v))
        .bind(patch.current_value.is_some())
        .bind(patch.current_value.as_ref().and_then(|v| *v))
        .bind(patch.quantity)
        .bind(patch.cost_basis)
        .bind(patch.notes.is_some())
        .bind(patch.notes.as_ref().and_then(|v| v.as_deref()))
        .bind(&patch.id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| StorageError::NotFound(format!("finance_investment {}", patch.id)))?;
        Ok(row)
    }

    /// Set the current price and value for an investment (convenience method for price refresh).
    pub async fn update_price(
        &self,
        id: &str,
        current_price: i64,
        current_value: i64,
    ) -> Result<FinanceInvestmentRow, StorageError> {
        let row = sqlx::query_as::<_, FinanceInvestmentRow>(
            r#"
            UPDATE finance_investments
            SET current_price = ?, current_value = ?, updated_at = datetime('now')
            WHERE id = ?
            RETURNING *
            "#,
        )
        .bind(current_price)
        .bind(current_value)
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| StorageError::NotFound(format!("finance_investment {id}")))?;
        Ok(row)
    }

    /// List investments matching the filter, ordered by created_at.
    pub async fn list_investments(
        &self,
        filter: &FinanceInvestmentFilter,
    ) -> Result<Vec<FinanceInvestmentRow>, StorageError> {
        let mut qb =
            sqlx::QueryBuilder::<sqlx::Sqlite>::new("SELECT * FROM finance_investments WHERE TRUE");

        if let Some(ref portfolio_id) = filter.portfolio_id {
            qb.push(" AND portfolio_id = ");
            qb.push_bind(portfolio_id);
        }

        if let Some(ref asset_type) = filter.asset_type {
            qb.push(" AND asset_type = ");
            qb.push_bind(asset_type);
        }

        if let Some(has_symbol) = filter.has_symbol {
            if has_symbol {
                qb.push(" AND symbol IS NOT NULL");
            } else {
                qb.push(" AND symbol IS NULL");
            }
        }

        qb.push(" ORDER BY created_at");

        let rows = qb
            .build_query_as::<FinanceInvestmentRow>()
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// List all investments that have a non-null symbol (for batch price refresh).
    pub async fn list_with_symbols(&self) -> Result<Vec<FinanceInvestmentRow>, StorageError> {
        self.list_investments(&FinanceInvestmentFilter {
            has_symbol: Some(true),
            ..Default::default()
        })
        .await
    }

    /// Delete an investment and cascade-delete all its transactions.
    pub async fn delete_investment(&self, id: &str) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM finance_investments WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    // -----------------------------------------------------------------------
    // Investment Transactions
    // -----------------------------------------------------------------------

    /// Insert an investment transaction. Returns the inserted row.
    pub async fn add_investment_tx(
        &self,
        row: &FinanceInvestmentTxRow,
    ) -> Result<FinanceInvestmentTxRow, StorageError> {
        let inserted = sqlx::query_as::<_, FinanceInvestmentTxRow>(
            r#"
            INSERT INTO finance_investment_transactions (
                id, investment_id, tx_type, quantity, price_per_unit,
                total_amount, currency, fees, tx_date, notes, created_at
            ) VALUES (
                ?, ?, ?, ?, ?,
                ?, ?, ?, ?, ?, ?
            )
            RETURNING *
            "#,
        )
        .bind(&row.id)
        .bind(&row.investment_id)
        .bind(&row.tx_type)
        .bind(row.quantity)
        .bind(row.price_per_unit)
        .bind(row.total_amount)
        .bind(&row.currency)
        .bind(row.fees)
        .bind(row.tx_date)
        .bind(&row.notes)
        .bind(row.created_at)
        .fetch_one(&self.pool)
        .await?;
        Ok(inserted)
    }

    /// List all investment transactions for an investment, ordered by tx_date.
    pub async fn list_investment_txs(
        &self,
        investment_id: &str,
    ) -> Result<Vec<FinanceInvestmentTxRow>, StorageError> {
        let rows = sqlx::query_as::<_, FinanceInvestmentTxRow>(
            "SELECT * FROM finance_investment_transactions WHERE investment_id = ? ORDER BY tx_date, created_at",
        )
        .bind(investment_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // -----------------------------------------------------------------------
    // Aggregation
    // -----------------------------------------------------------------------

    /// Aggregate summary for a single portfolio (total cost basis, current value, holding count).
    pub async fn portfolio_summary(
        &self,
        portfolio_id: &str,
    ) -> Result<PortfolioSummaryRow, StorageError> {
        let row = sqlx::query_as::<_, PortfolioSummaryRow>(
            r#"
            SELECT
                p.id                                              AS portfolio_id,
                COALESCE(SUM(i.cost_basis), 0)                    AS total_cost_basis,
                COALESCE(SUM(i.current_value), 0)                 AS total_current_value,
                COUNT(i.id)                                       AS holding_count
            FROM finance_portfolios p
            LEFT JOIN finance_investments i ON i.portfolio_id = p.id
            WHERE p.id = ?
            GROUP BY p.id
            "#,
        )
        .bind(portfolio_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| StorageError::NotFound(format!("finance_portfolio {portfolio_id}")))?;
        Ok(row)
    }

    /// Sum `current_value` of investments with non-null values, grouped by currency.
    /// Returns `(currency, total_value)` pairs.
    pub async fn total_value_by_currency(&self) -> Result<Vec<(String, i64)>, StorageError> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT currency, COALESCE(SUM(current_value), 0) AS total
            FROM finance_investments
            WHERE current_value IS NOT NULL
            GROUP BY currency
            ORDER BY currency
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}
