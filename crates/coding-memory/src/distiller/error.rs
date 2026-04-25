//! Distiller-scoped error taxonomy.
//!
//! The Distiller is a write-path subsystem; it must never silently swallow
//! failures. Every failure mode has an explicit variant so callers can
//! choose retry/skip/abort policy and Mirror subscribers can categorize.

use thiserror::Error;

/// Errors produced by the Distiller pipeline (Phase A / B / C / writer).
#[derive(Debug, Error)]
#[allow(missing_docs)]
pub enum DistillerError {
    /// The LLM provider timed out while synthesizing observations.
    #[error("LLM timeout after {timeout_ms}ms")]
    LlmTimeout { timeout_ms: u64 },

    /// The LLM produced text/tool-call JSON that couldn't be decoded.
    #[error("LLM malformed tool call: {detail}")]
    LlmMalformed { detail: String },

    /// The provider manager returned an error (configured provider unavailable etc.).
    #[error("LLM provider error: {detail}")]
    LlmProvider { detail: String },

    /// A write was attempted with empty `source_events` provenance.
    #[error("provenance missing: source_events is empty")]
    ProvenanceMissing,

    /// The event body couldn't be serialized / deserialized.
    #[error("event decode failure: {detail}")]
    EventDecode { detail: String },

    /// An underlying storage operation failed.
    #[error("storage error: {detail}")]
    Storage { detail: String },

    /// The turn is already being processed by another cycle.
    #[error("turn already in flight")]
    TurnInFlight,

    /// A transient failure — caller should retry on next cycle.
    #[error("transient: {detail}")]
    Transient { detail: String },
}

impl From<DistillerError> for common::KlyntbotError {
    fn from(e: DistillerError) -> Self {
        common::KlyntbotError::Storage(format!("distiller: {e}"))
    }
}
