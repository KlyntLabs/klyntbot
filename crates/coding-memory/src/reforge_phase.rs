//! Reforge coding phases — 2.5 (Coding Synthesis) + 3.5 (Rule Artifact Generation).
//!
//! Phase 1 lands type + stub method surface. Bodies land in Phase 5.
//!
//! `ReforgeWriter` wrapper must be used by these phases; it rejects DELETE
//! at runtime to enforce the "Reforge-never-deletes-raw" invariant.

use crate::error::NotImplementedInPhase;
use async_trait::async_trait;
use common::{KlyntbotError, Result};
use std::path::PathBuf;

/// A single Reforge phase run — plugs into the existing nightly cycle.
#[async_trait]
pub trait ReforgePhaseRun: Send + Sync {
    /// Human name for logging / mirror snippets.
    fn name(&self) -> &'static str;

    /// Run exactly one instance of the phase. Phases are `Result<()>`-isolated
    /// — a failure logs to `mirror_snippets` and does not cascade.
    async fn run(&self) -> Result<()>;
}

/// Phase 2.5 — Coding Synthesis.
///
/// Consumes: sessions + new `FixAttempt`s + causal edges + active
/// `WorkflowPattern`s. Emits: `ExtractPattern`, `ExtractFailurePattern`,
/// `PromoteToProblemClass`, `PromoteToProjectUnderstanding`, `PromoteToUserHabit`,
/// `PromoteToProblemSolutionPattern`.
#[derive(Debug, Default)]
pub struct CodingSynthesisPhase {
    /// Phase-5 wiring carries provider-manager handle + cognitive repos.
    _phase_stub: (),
}

#[async_trait]
impl ReforgePhaseRun for CodingSynthesisPhase {
    fn name(&self) -> &'static str {
        "reforge.coding_synthesis"
    }

    async fn run(&self) -> Result<()> {
        Err(phase(5))
    }
}

/// Phase 3.5 — Rule Artifact Generation.
///
/// Reads active patterns/preferences/understanding with `confidence ≥ 0.7`
/// and `stability ≥ 0.5`. Writes managed-block sections of per-repo
/// `CLAUDE.md` / `AGENTS.md` / `.cursorrules`. Skips `high` and `excluded`
/// sensitivity tiers.
#[derive(Debug, Default)]
pub struct RuleArtifactGenerationPhase {
    /// Phase-5 wiring carries repo discovery + managed-block writer.
    _phase_stub: (),
}

#[async_trait]
impl ReforgePhaseRun for RuleArtifactGenerationPhase {
    fn name(&self) -> &'static str {
        "reforge.rule_artifact_generation"
    }

    async fn run(&self) -> Result<()> {
        Err(phase(5))
    }
}

/// Managed-block markers. Opaque delimiters the rule writer preserves.
pub const MANAGED_BLOCK_START: &str = "<!-- klyntbot:managed:start";
/// End marker.
pub const MANAGED_BLOCK_END: &str = "<!-- klyntbot:managed:end -->";

/// Which on-disk rule artifact is being generated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleArtifact {
    /// `<repo_root>/CLAUDE.md`.
    ClaudeMd,
    /// `<repo_root>/AGENTS.md`.
    AgentsMd,
    /// `<repo_root>/.cursorrules`.
    CursorRules,
    /// `<repo_root>/.continue/rules/klyntbot.md`.
    ContinueRules,
}

impl RuleArtifact {
    /// Relative path under a repo root for this artifact.
    #[must_use]
    pub fn relative_path(self) -> PathBuf {
        match self {
            RuleArtifact::ClaudeMd => PathBuf::from("CLAUDE.md"),
            RuleArtifact::AgentsMd => PathBuf::from("AGENTS.md"),
            RuleArtifact::CursorRules => PathBuf::from(".cursorrules"),
            RuleArtifact::ContinueRules => PathBuf::from(".continue/rules/klyntbot.md"),
        }
    }
}

fn phase(p: u8) -> KlyntbotError {
    KlyntbotError::NotImplemented(format!("{:?}", NotImplementedInPhase::new(p)))
}
