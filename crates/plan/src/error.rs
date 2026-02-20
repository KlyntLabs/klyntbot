//! Plan-specific error types.

use thiserror::Error;

/// Plan-specific errors
#[derive(Error, Debug)]
pub enum PlanError {
    #[error("Plan not found: {0}")]
    NotFound(String),

    #[error("Plan generation failed: {0}")]
    GenerationFailed(String),

    #[error("Invalid plan state: {0}")]
    InvalidState(String),

    #[error("Execution stalled at step {step_index}: {reason}")]
    ExecutionStalled { step_index: usize, reason: String },

    #[error("Backtrack limit reached at step {0}")]
    BacktrackLimitReached(usize),

    #[error("Plan store error: {0}")]
    StoreFailed(String),
}

impl From<PlanError> for common::KlyntbotError {
    fn from(e: PlanError) -> Self {
        common::KlyntbotError::Plan(e.to_string())
    }
}
