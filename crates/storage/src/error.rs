//! Storage-specific error types.

use thiserror::Error;

/// Errors originating from the storage layer.
#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Database error: {0}")]
    Sqlx(#[from] sqlx::Error),

    #[error("Migration error: {0}")]
    Migration(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Vector store error: {0}")]
    Vector(String),
}

impl From<sqlx::migrate::MigrateError> for StorageError {
    fn from(e: sqlx::migrate::MigrateError) -> Self {
        StorageError::Migration(e.to_string())
    }
}

/// Extension trait for ergonomic `NotFound` conversion on `Option<T>`.
///
/// ```ignore
/// fetch_optional(...).await?.ok_or_not_found("todo 42")?;
/// ```
pub trait OptionExt<T> {
    fn ok_or_not_found(self, label: &str) -> Result<T, StorageError>;
}

impl<T> OptionExt<T> for Option<T> {
    fn ok_or_not_found(self, label: &str) -> Result<T, StorageError> {
        self.ok_or_else(|| StorageError::NotFound(label.to_string()))
    }
}

impl From<StorageError> for common::KlyntbotError {
    fn from(e: StorageError) -> Self {
        match e {
            StorageError::NotFound(msg) => common::KlyntbotError::StorageNotFound(msg),
            StorageError::Conflict(msg) => common::KlyntbotError::StorageConflict(msg),
            other => common::KlyntbotError::Storage(other.to_string()),
        }
    }
}
