//! Dependency-inversion handlers for LLM-backed coding phases. Filled in by Task 5.

use crate::reforge::types::{CodingSynthesisInput, CodingSynthesisOutput, RuleArtifactInput, RuleArtifactOutput};
use async_trait::async_trait;
use common::Result;

/// Phase 2.5 LLM seam.
#[async_trait]
pub trait CodingSynthesisHandler: Send + Sync {
    /// Run synthesis.
    async fn synthesize_coding(&self, input: &CodingSynthesisInput) -> Result<CodingSynthesisOutput>;
}

/// Phase 3.5 LLM seam.
#[async_trait]
pub trait RuleArtifactsHandler: Send + Sync {
    /// Generate a single rule artifact body.
    async fn synthesize_artifact(&self, input: &RuleArtifactInput) -> Result<RuleArtifactOutput>;
}
