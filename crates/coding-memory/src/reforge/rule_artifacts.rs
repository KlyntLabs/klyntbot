//! Phase 3.5 — Rule Artifact Generation. Filled in by Task 8.

use crate::error::NotImplementedInPhase;
use crate::reforge::types::CodingPhaseHandlers;
use common::Result;

/// Orchestrator for Phase 3.5.
#[derive(Debug)]
pub struct RuleArtifactGenerationPhase;

impl RuleArtifactGenerationPhase {
    /// Run the phase.
    pub async fn run(_handlers: &CodingPhaseHandlers<'_>) -> Result<()> {
        Err(common::KlyntbotError::NotImplemented(format!(
            "{:?}",
            NotImplementedInPhase::new(5)
        )))
    }
}
