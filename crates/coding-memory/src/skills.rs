//! Scope-aware `SkillStore` extension + project-scoped evolving skills.
//!
//! Phase 1 lands the types. Phase 5 wires Reforge's Phase 3.5 sub-phase
//! to auto-synthesize `SKILL.md` files from `WorkflowPattern`s.

use crate::reforge::types::ProjectSkillSpec;
use crate::skill_evolver::{
    detect_candidates, supersede_outdated_versions, write_skill_md, JournalArgs, SkillWriteOutcome,
};
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

/// Real Phase-5 implementation of [`ProjectSkillEvolver`].
#[derive(Debug, Clone)]
pub struct ProjectSkillEvolverImpl {
    /// Storage pool for DB queries.
    pool: storage::StoragePool,
    /// Base directory for private skill storage.
    base_dir: PathBuf,
}

impl ProjectSkillEvolverImpl {
    /// Create a new evolver.
    pub fn new(pool: storage::StoragePool, base_dir: PathBuf) -> Self {
        Self { pool, base_dir }
    }
}

#[async_trait]
impl ProjectSkillEvolver for ProjectSkillEvolverImpl {
    async fn evolve(&self, repo_id: &str) -> Result<Vec<SkillSynthesisResult>> {
        let candidates = detect_candidates(&self.pool, repo_id).await?;
        let mut results = Vec::new();

        for candidate in candidates {
            let skill_id = crate::skill_evolver::write::sanitize(&candidate.rule_text);
            let spec = ProjectSkillSpec {
                skill_id: skill_id.clone(),
                repo_id: repo_id.to_string(),
                name: candidate.rule_text.clone(),
                description: format!(
                    "Auto-detected workflow pattern (confidence {:.0}%)",
                    candidate.confidence * 100.0
                ),
                when_to_use: vec![],
                procedure: candidate.rule_text.clone(),
                references: vec![],
                anchored_symbols: vec![],
                effectiveness: candidate.effectiveness,
            };

            let skill_path = self.base_dir.join(&skill_id).join("SKILL.md");
            let cycle_id = format!("evolve-{}", Uuid::new_v4());

            let outcome = write_skill_md(
                &self.pool,
                &JournalArgs {
                    repo_id: repo_id.to_string(),
                    skill_path: skill_path.clone(),
                    cycle_id,
                    spec,
                },
            )
            .await?;

            match outcome {
                SkillWriteOutcome::Written { version, path } => {
                    let version_id = Uuid::new_v4();
                    let content = match tokio::fs::read_to_string(&path).await {
                        Ok(c) => c,
                        Err(e) => {
                            return Err(KlyntbotError::Storage(format!(
                                "read back skill file: {e}"
                            )))
                        }
                    };

                    sqlx::query(
                        "INSERT INTO skill_versions \
                         (id, skill_name, version, file_path, content, source, scope, scope_repo_id, source_pattern_id, status) \
                         VALUES (?1, ?2, ?3, ?4, ?5, 'evolver', 'project', ?6, ?7, 'active')",
                    )
                    .bind(version_id.to_string())
                    .bind(&skill_id)
                    .bind(version)
                    .bind(path.to_string_lossy().to_string())
                    .bind(content)
                    .bind(repo_id)
                    .bind(&candidate.id)
                    .execute(self.pool.inner())
                    .await
                    .map_err(|e| KlyntbotError::Storage(format!("skill_versions insert: {e}")))?;

                    supersede_outdated_versions(&self.pool, &skill_id).await?;

                    results.push(SkillSynthesisResult {
                        skill_id: SkillId(skill_id),
                        skill_path: path,
                        version_id,
                        effectiveness: candidate.effectiveness,
                    });
                }
                SkillWriteOutcome::Skipped { .. } => {}
            }
        }

        Ok(results)
    }
}
