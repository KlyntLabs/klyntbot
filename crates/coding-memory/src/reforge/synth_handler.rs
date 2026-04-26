//! Dependency-inversion handlers for LLM-backed coding phases. Filled in by Task 5.

use crate::reforge::types::{
    CodingSynthesisInput, CodingSynthesisOutput, ProjectSkillSpec, RuleArtifactInput,
    RuleArtifactOutput,
};
use async_trait::async_trait;
use common::Result;

/// Phase 2.5 LLM seam.
#[async_trait]
pub trait CodingSynthesisHandler: Send + Sync {
    /// Run synthesis.
    async fn synthesize_coding(
        &self,
        input: &CodingSynthesisInput,
    ) -> Result<CodingSynthesisOutput>;
}

/// Phase 3.5 LLM seam.
#[async_trait]
pub trait RuleArtifactsHandler: Send + Sync {
    /// Generate a single rule artifact body.
    async fn synthesize_artifact(&self, input: &RuleArtifactInput) -> Result<RuleArtifactOutput>;

    /// Synthesize a `ProjectSkillSpec` from a workflow pattern rule text.
    async fn synthesize_skill(&self, rule_text: &str, repo_id: &str) -> Result<ProjectSkillSpec> {
        let skill_id = rule_text
            .to_lowercase()
            .replace(|c: char| !c.is_alphanumeric() && c != ' ', "")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join("-");
        Ok(ProjectSkillSpec {
            skill_id,
            repo_id: repo_id.to_string(),
            name: rule_text.to_string(),
            description: "Auto-detected workflow pattern".to_string(),
            when_to_use: vec![],
            procedure: rule_text.to_string(),
            references: vec![],
            anchored_symbols: vec![],
            effectiveness: 0.7,
        })
    }
}
