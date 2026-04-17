//! Repository for the `finance_accounts` table.

use crate::rows::finance::{FinanceAccountPatch, FinanceAccountRow};

crud_repo!(
    FinanceAccountRepo,
    "finance_accounts",
    FinanceAccountRow,
    "finance_account"
);

impl FinanceAccountRepo {
    // -----------------------------------------------------------------------
    // CRUD (add + update are hand-written — too much per-repo variation)
    // -----------------------------------------------------------------------

    /// Insert a new account. Returns the inserted row.
    pub async fn add(
        &self,
        row: &FinanceAccountRow,
    ) -> Result<FinanceAccountRow, crate::error::StorageError> {
        let inserted = sqlx::query_as::<_, FinanceAccountRow>(
            r#"
            INSERT INTO finance_accounts (
                id, name, account_type, currency, balance,
                institution, notes, is_archived, created_at, updated_at,
                base_balance, base_currency, exchange_rate
            ) VALUES (
                ?, ?, ?, ?, ?,
                ?, ?, ?, ?, ?,
                ?, ?, ?
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
        .bind(row.base_balance)
        .bind(&row.base_currency)
        .bind(row.exchange_rate)
        .fetch_one(&self.pool)
        .await?;

        Ok(inserted)
    }

    /// Update mutable fields on an account. Only non-`None` fields are changed.
    pub async fn update(
        &self,
        patch: &FinanceAccountPatch,
    ) -> Result<FinanceAccountRow, crate::error::StorageError> {
        let now: crate::sqlite_types::SqlTs = jiff::Timestamp::now().into();
        let row = sqlx::query_as::<_, FinanceAccountRow>(
            r#"
            UPDATE finance_accounts SET
                name          = COALESCE(?, name),
                balance       = COALESCE(?, balance),
                institution   = CASE WHEN ? THEN ? ELSE institution END,
                notes         = CASE WHEN ? THEN ? ELSE notes END,
                is_archived   = COALESCE(?, is_archived),
                base_balance  = COALESCE(?, base_balance),
                base_currency = COALESCE(?, base_currency),
                exchange_rate = COALESCE(?, exchange_rate),
                updated_at    = ?
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
        .bind(patch.base_balance)
        .bind(patch.base_currency.as_deref())
        .bind(patch.exchange_rate)
        .bind(now)
        .bind(&patch.id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| {
            crate::error::StorageError::NotFound(format!("finance_account {}", patch.id))
        })?;

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
    pub async fn total_balance_by_currency(
        &self,
    ) -> Result<Vec<(String, i64)>, crate::error::StorageError> {
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

    /// Sum `base_balance` of all non-archived accounts whose `base_currency` matches.
    ///
    /// This gives a single consolidated total in the user's home currency.
    pub async fn total_base_balance(
        &self,
        base_currency: &str,
    ) -> Result<i64, crate::error::StorageError> {
        let row = sqlx::query_as::<_, (i64,)>(
            "SELECT COALESCE(SUM(base_balance), 0) FROM finance_accounts WHERE is_archived = FALSE AND base_currency = ?",
        )
        .bind(base_currency)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
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
        let now: crate::sqlite_types::SqlTs = jiff::Timestamp::now().into();
        let row = sqlx::query_as::<_, FinanceAccountRow>(
            r#"
            UPDATE finance_accounts
            SET balance = balance + ?,
                base_balance = CAST(ROUND((balance + ?) * exchange_rate) AS INTEGER),
                updated_at = ?
            WHERE id = ?
            RETURNING *
            "#,
        )
        .bind(delta)
        .bind(delta)
        .bind(now)
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| crate::error::StorageError::NotFound(format!("finance_account {id}")))?;
        Ok(row)
    }
}
