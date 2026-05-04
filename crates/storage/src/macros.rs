//! Declarative macros for reducing CRUD boilerplate in repository modules.

/// Generate a repository struct with common CRUD methods.
///
/// Generates:
/// - `struct $repo { pool: SqlitePool }` with `Debug, Clone`
/// - `fn new(pool) -> Self`
/// - `async fn get(id) -> Result<Option<Row>>`
/// - `async fn get_or_err(id) -> Result<Row>`
/// - `async fn delete(id) -> Result<bool>` (default variant)
///
/// Use `@no_delete` to omit the `delete` method (for repos with custom delete
/// signatures, e.g. returning the deleted row).
///
/// # Example
///
/// ```ignore
/// crud_repo!(FinanceAccountRepo, "finance_accounts", FinanceAccountRow, "finance_account");
///
/// // For a repo with a custom delete:
/// crud_repo!(@no_delete FinanceTransactionRepo, "finance_transactions", FinanceTransactionRow, "finance_transaction");
/// ```
macro_rules! crud_repo {
    // Default: struct + new + get + get_or_err + delete(bool)
    ($repo:ident, $table:expr, $row:ty, $label:expr) => {
        crud_repo!(@base $repo, $table, $row, $label);

        impl $repo {
            /// Delete a row by id. Returns `true` if the row existed and was deleted.
            pub async fn delete(&self, id: &str) -> Result<bool, $crate::error::StorageError> {
                let result = sqlx::query(concat!("DELETE FROM ", $table, " WHERE id = ?"))
                    .bind(id)
                    .execute(&self.pool)
                    .await?;
                Ok(result.rows_affected() > 0)
            }
        }
    };

    // Without delete — caller provides their own.
    (@no_delete $repo:ident, $table:expr, $row:ty, $label:expr) => {
        crud_repo!(@base $repo, $table, $row, $label);
    };

    // Base: struct + new + get + get_or_err
    (@base $repo:ident, $table:expr, $row:ty, $label:expr) => {
        #[derive(Debug, Clone)]
        pub struct $repo {
            pool: sqlx::SqlitePool,
        }

        impl $repo {
            pub fn new(pool: sqlx::SqlitePool) -> Self {
                Self { pool }
            }

            /// Get a single row by id. Returns `None` if not found.
            pub async fn get(
                &self,
                id: &str,
            ) -> Result<Option<$row>, $crate::error::StorageError> {
                let row = sqlx::query_as::<_, $row>(
                    concat!("SELECT * FROM ", $table, " WHERE id = ?"),
                )
                .bind(id)
                .fetch_optional(&self.pool)
                .await?;
                Ok(row)
            }

            /// Get a single row by id, returning `StorageError::NotFound` if missing.
            pub async fn get_or_err(
                &self,
                id: &str,
            ) -> Result<$row, $crate::error::StorageError> {
                self.get(id)
                    .await?
                    .ok_or_else(|| {
                        $crate::error::StorageError::NotFound(format!(
                            concat!($label, " {}"),
                            id
                        ))
                    })
            }
        }
    };
}

/// Generate `focus`, `unfocus`, and `list_focused` methods for a repo.
///
/// Generate focus-slot methods (`focus`, `unfocus`, `list_focused`) for a repo.
/// The table name and row type are parameterized.
macro_rules! focus_impl {
    ($repo:ty, $table:expr, $row:ty) => {
        impl $repo {
            /// Focus an item. Returns true if the focus was set, false if at max_slots.
            pub async fn focus(
                &self,
                id: &str,
                max_slots: i64,
                deadline: Option<jiff::Timestamp>,
            ) -> Result<bool, $crate::error::StorageError> {
                let result = sqlx::query(concat!(
                    "UPDATE ", $table,
                    " SET focused_at = (unixepoch('now') * 1000), focus_deadline = ?3, updated_at = (unixepoch('now') * 1000)",
                    " WHERE id = ?1 AND focused_at IS NULL",
                    " AND (SELECT COUNT(*) FROM ", $table, " WHERE focused_at IS NOT NULL) < ?2",
                ))
                .bind(id)
                .bind(max_slots)
                .bind(deadline.map(|t| t.as_millisecond()))
                .execute(&self.pool)
                .await?;
                Ok(result.rows_affected() > 0)
            }

            /// Unfocus an item. Returns `true` only if the row was actually focused.
            pub async fn unfocus(&self, id: &str) -> Result<bool, $crate::error::StorageError> {
                let result = sqlx::query(concat!(
                    "UPDATE ", $table,
                    " SET focused_at = NULL, focus_deadline = NULL, updated_at = (unixepoch('now') * 1000)",
                    " WHERE id = ?1 AND focused_at IS NOT NULL",
                ))
                .bind(id)
                .execute(&self.pool)
                .await?;
                Ok(result.rows_affected() > 0)
            }

            /// List currently focused items.
            pub async fn list_focused(&self) -> Result<Vec<$row>, $crate::error::StorageError> {
                let rows = sqlx::query_as::<_, $row>(
                    concat!("SELECT * FROM ", $table, " WHERE focused_at IS NOT NULL ORDER BY focused_at"),
                )
                .fetch_all(&self.pool)
                .await?;
                Ok(rows)
            }
        }
    };
}

/// Generate a `delete_older_than` retention method.
///
/// Four repos (`StrategyRepo`, `OutcomeRepo`, `InteractionLogRepo`, `ToolUsageRepo`)
/// have byte-for-byte identical implementations of this pattern.
macro_rules! delete_older_than_impl {
    ($table:expr, $ts_col:expr) => {
        /// Delete rows older than `days` days. Returns count of deleted rows.
        pub async fn delete_older_than(
            &self,
            days: i64,
            now: jiff::Timestamp,
        ) -> Result<u64, $crate::error::StorageError> {
            let cutoff = now - jiff::SignedDuration::from_hours(days * 24);
            let result = sqlx::query(concat!("DELETE FROM ", $table, " WHERE ", $ts_col, " < ?1"))
                .bind(cutoff.as_millisecond())
                .execute(&self.pool)
                .await?;
            Ok(result.rows_affected())
        }
    };
}

/// Generate a `get_by_ids` batch-fetch method.
///
/// Generate a `get_by_ids` batch-fetch method. Early-return
/// on empty, build an IN-clause with `QueryBuilder`, `fetch_all`.
macro_rules! get_by_ids_impl {
    ($table:expr, $row:ty) => {
        /// Fetch rows by a list of IDs. Missing IDs are silently skipped.
        pub async fn get_by_ids(
            &self,
            ids: &[String],
        ) -> Result<Vec<$row>, $crate::error::StorageError> {
            if ids.is_empty() {
                return Ok(Vec::new());
            }
            let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new(concat!(
                "SELECT * FROM ",
                $table,
                " WHERE id IN ("
            ));
            let mut sep = qb.separated(", ");
            for id in ids {
                sep.push_bind(id);
            }
            qb.push(")");
            let rows = qb.build_query_as::<$row>().fetch_all(&self.pool).await?;
            Ok(rows)
        }
    };
}

/// Escape a string for use in a SQL `LIKE` pattern (with `\` as escape char).
///
/// Used by `search_by_keyword` in `TaskRepo` and `FinanceTransactionRepo`.
pub fn escape_like(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for ch in s.chars() {
        if matches!(ch, '\\' | '%' | '_') {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}
