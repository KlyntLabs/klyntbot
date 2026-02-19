//! Storage-specific error types.

use thiserror::Error;

/// Errors originating from the storage layer.
#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Database error: {0}")]
    Sqlx(#[from] sqlx::Error),

    #[error("Migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Conflict: {0}")]
    Conflict(String),
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
