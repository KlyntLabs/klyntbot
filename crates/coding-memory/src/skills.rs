//! Scope-aware `SkillStore` extension + project-scoped evolving skills.
//!
//! Phase 1 lands the types. Phase 5 wires Reforge's Phase 3.5 sub-phase
//! to auto-synthesize `SKILL.md` files from `WorkflowPattern`s.

use crate::error::NotImplementedInPhase;
use async_trait::async_trait;
use common::{KlyntbotError, Result};
use std::path::PathBuf;
use uuid::Uuid;

/// Where a project skill is stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectSkillLocation {
    /// Private default: `~/.klyntbot/project-skills/<repo>/<skill>/SKILL.md`.
    Private,
    /// Team-shared: `<repo_root>/.klyntbot/skills/<skill>/SKILL.md`.
    Team,
}

/// Scope of a skill — either global or bound to one repo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillScope {
    /// Applies everywhere.
    Global,
    /// Applies to one repo only.
    Repo {
        /// Canonical repo id.
        repo_id: String,
    },
}

/// Identifier for a skill.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillId(pub String);

/// Project-skill evolution driver — reads `WorkflowPattern`s, synthesizes
/// `SKILL.md`, writes via existing `SkillFileManager`.
#[async_trait]
pub trait ProjectSkillEvolver: Send + Sync {
    /// Run one evolution pass for a given repo.
    async fn evolve(&self, repo_id: &str) -> Result<Vec<SkillSynthesisResult>>;
}

/// Outcome of one skill synthesis.
#[derive(Debug, Clone)]
pub struct SkillSynthesisResult {
    /// Skill id.
    pub skill_id: SkillId,
    /// Absolute SKILL.md path.
    pub skill_path: PathBuf,
    /// Version row id.
    pub version_id: Uuid,
    /// Starting effectiveness score.
    pub effectiveness: f32,
}

/// Phase-1 stub evolver.
#[derive(Debug, Default)]
pub struct PhaseStubEvolver;

#[async_trait]
impl ProjectSkillEvolver for PhaseStubEvolver {
    async fn evolve(&self, _repo_id: &str) -> Result<Vec<SkillSynthesisResult>> {
        Err(phase(5))
    }
}

fn phase(p: u8) -> KlyntbotError {
    KlyntbotError::NotImplemented(format!("{:?}", NotImplementedInPhase::new(p)))
}
