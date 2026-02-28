//! Repository for the `finance_accounts` table.

use crate::rows::finance::{FinanceAccountPatch, FinanceAccountRow};

crud_repo!(FinanceAccountRepo, "finance_accounts", FinanceAccountRow, "finance_account");

impl FinanceAccountRepo {
    // -----------------------------------------------------------------------
    // CRUD (add + update are hand-written — too much per-repo variation)
    // -----------------------------------------------------------------------

    /// Insert a new account. Returns the inserted row.
    pub async fn add(&self, row: &FinanceAccountRow) -> Result<FinanceAccountRow, crate::error::StorageError> {
        let inserted = sqlx::query_as::<_, FinanceAccountRow>(
            r#"
            INSERT INTO finance_accounts (
                id, name, account_type, currency, balance,
                institution, notes, is_archived, created_at, updated_at
            ) VALUES (
                ?, ?, ?, ?, ?,
                ?, ?, ?, ?, ?
            )
            RETURNING *
            "#,
        )
        .bind(&row.id)
        .bind(&row.name)
        .bind(&row.account_type)
        .bind(&row.currency)
        .bind(row.balance)
        .bind(&row.institution)
        .bind(&row.notes)
        .bind(row.is_archived)
        .bind(row.created_at)
        .bind(row.updated_at)
        .fetch_one(&self.pool)
        .await?;

        Ok(inserted)
    }

    /// Update mutable fields on an account. Only non-`None` fields are changed.
    pub async fn update(
        &self,
        patch: &FinanceAccountPatch,
    ) -> Result<FinanceAccountRow, crate::error::StorageError> {
        let now = chrono::Utc::now();
        let row = sqlx::query_as::<_, FinanceAccountRow>(
            r#"
            UPDATE finance_accounts SET
                name        = COALESCE(?, name),
                balance     = COALESCE(?, balance),
                institution = CASE WHEN ? THEN ? ELSE institution END,
                notes       = CASE WHEN ? THEN ? ELSE notes END,
                is_archived = COALESCE(?, is_archived),
                updated_at  = ?
            WHERE id = ?
            RETURNING *
            "#,
        )
        .bind(&patch.name)
        .bind(patch.balance)
        .bind(patch.institution.is_some())
        .bind(patch.institution.as_ref().and_then(|v| v.as_deref()))
        .bind(patch.notes.is_some())
        .bind(patch.notes.as_ref().and_then(|v| v.as_deref()))
        .bind(patch.is_archived)
        .bind(now)
        .bind(&patch.id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| crate::error::StorageError::NotFound(format!("finance_account {}", patch.id)))?;

        Ok(row)
    }

    // -----------------------------------------------------------------------
    // Listing
    // -----------------------------------------------------------------------

    /// List all accounts. Pass `include_archived = true` to include archived accounts.
    pub async fn list(
        &self,
        include_archived: bool,
    ) -> Result<Vec<FinanceAccountRow>, crate::error::StorageError> {
        let rows = if include_archived {
            sqlx::query_as::<_, FinanceAccountRow>(
                "SELECT * FROM finance_accounts ORDER BY created_at",
            )
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, FinanceAccountRow>(
                "SELECT * FROM finance_accounts WHERE is_archived = FALSE ORDER BY created_at",
            )
            .fetch_all(&self.pool)
            .await?
        };
        Ok(rows)
    }

    /// List non-archived accounts with the given currency.
    pub async fn list_by_currency(
        &self,
        currency: &str,
    ) -> Result<Vec<FinanceAccountRow>, crate::error::StorageError> {
        let rows = sqlx::query_as::<_, FinanceAccountRow>(
            r#"
            SELECT * FROM finance_accounts
            WHERE currency = ? AND is_archived = FALSE
            ORDER BY created_at
            "#,
        )
        .bind(currency)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // -----------------------------------------------------------------------
    // Aggregation
    // -----------------------------------------------------------------------

    /// Sum balances of non-archived accounts, grouped by currency.
    /// Returns a vec of `(currency, total_balance)` pairs.
    pub async fn total_balance_by_currency(&self) -> Result<Vec<(String, i64)>, crate::error::StorageError> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT currency, COALESCE(SUM(balance), 0) AS total
            FROM finance_accounts
            WHERE is_archived = FALSE
            GROUP BY currency
            ORDER BY currency
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // -----------------------------------------------------------------------
    // Balance Operations
    // -----------------------------------------------------------------------

    /// Add `delta` to the account's balance (may be negative). Returns the updated row.
    pub async fn adjust_balance(
        &self,
        id: &str,
        delta: i64,
    ) -> Result<FinanceAccountRow, crate::error::StorageError> {
        let now = chrono::Utc::now();
        let row = sqlx::query_as::<_, FinanceAccountRow>(
            r#"
            UPDATE finance_accounts
            SET balance = balance + ?, updated_at = ?
            WHERE id = ?
            RETURNING *
            "#,
        )
        .bind(delta)
        .bind(now)
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| crate::error::StorageError::NotFound(format!("finance_account {id}")))?;
        Ok(row)
    }
}
