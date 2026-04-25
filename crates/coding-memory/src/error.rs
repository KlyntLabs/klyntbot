//! Phase-scoped stub error surface.

use thiserror::Error;

/// Top-level error for `coding-memory` stubs.
#[derive(Debug, Error)]
pub enum CodingMemoryError {
    /// Method is not yet implemented — it becomes available in `required_phase`.
    #[error("coding-memory operation not implemented until phase {}", .0.required_phase)]
    NotImplemented(NotImplementedInPhase),
}

impl From<CodingMemoryError> for common::KlyntbotError {
    fn from(e: CodingMemoryError) -> Self {
        common::KlyntbotError::NotImplemented(e.to_string())
    }
}

/// Indicates the phase that must be completed before this operation is wired.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotImplementedInPhase {
    /// The phase number (2-8) in which this method becomes non-stub.
    pub required_phase: u8,
}

impl NotImplementedInPhase {
    /// Construct a `NotImplementedInPhase` marker.
    #[must_use]
    pub const fn new(required_phase: u8) -> Self {
        Self { required_phase }
    }
}
