//! Phase 2.5 — Coding Synthesis. Filled in by Task 6.

use crate::error::NotImplementedInPhase;
use crate::reforge::types::CodingPhaseHandlers;
use common::Result;

/// Orchestrator for Phase 2.5.
#[derive(Debug)]
pub struct CodingSynthesisPhase;

impl CodingSynthesisPhase {
    /// Run the phase.
    pub async fn run(_handlers: &CodingPhaseHandlers<'_>) -> Result<()> {
        Err(common::KlyntbotError::NotImplemented(format!(
            "{:?}",
            NotImplementedInPhase::new(5)
        )))
    }
}
