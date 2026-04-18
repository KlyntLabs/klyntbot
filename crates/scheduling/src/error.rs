//! Cron-specific error types.

use thiserror::Error;

/// Cron-specific errors
#[derive(Error, Debug)]
pub enum CronError {
    #[error("Invalid cron expression: {0}")]
    InvalidExpression(String),

    #[error("Job not found: {0}")]
    JobNotFound(String),

    #[error("Job execution failed: {0}")]
    ExecutionFailed(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

impl From<CronError> for common::KlyntbotError {
    fn from(e: CronError) -> Self {
        common::KlyntbotError::Cron(e.to_string())
    }
}

use storage::error::StorageError;

/// Temporal scheduler errors.
#[derive(Debug, thiserror::Error)]
pub enum SchedulerError {
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
    #[error("rule error: {0}")]
    Rule(#[from] crate::temporal::rules::RuleError),
    #[error("rrule error: {0}")]
    Rrule(String),
    #[error("invalid state: {0}")]
    InvalidState(String),
}
