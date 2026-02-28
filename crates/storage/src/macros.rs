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
