//! Goal-specific error types.

use thiserror::Error;

/// Goal-specific errors
#[derive(Error, Debug)]
pub enum GoalError {
    #[error("Goal not found: {0}")]
    NotFound(String),

    #[error("Invalid goal state: {0}")]
    InvalidState(String),

    #[error("Goal store error: {0}")]
    StoreFailed(String),

    #[error("Goal validation failed: {0}")]
    ValidationFailed(String),
}

impl From<GoalError> for common::KlyntbotError {
    fn from(e: GoalError) -> Self {
        common::KlyntbotError::Goal(e.to_string())
    }
}
