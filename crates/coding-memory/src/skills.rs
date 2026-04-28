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
use providers::{ProviderManager, ProviderRole};
use std::path::PathBuf;
use std::sync::Arc;
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
#[derive(Clone)]
pub struct ProjectSkillEvolverImpl {
    /// Storage pool for DB queries.
    pool: storage::StoragePool,
    /// Base directory for private skill storage.
    base_dir: PathBuf,
    /// Optional provider manager for LLM-drafted SKILL.md content.
    provider_manager: Option<Arc<ProviderManager>>,
}

impl std::fmt::Debug for ProjectSkillEvolverImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProjectSkillEvolverImpl")
            .field("base_dir", &self.base_dir)
            .field("has_provider", &self.provider_manager.is_some())
            .finish()
    }
}

impl ProjectSkillEvolverImpl {
    /// Create a new evolver without LLM drafting (tests, offline mode).
    pub fn new(pool: storage::StoragePool, base_dir: PathBuf) -> Self {
        Self {
            pool,
            base_dir,
            provider_manager: None,
        }
    }

    /// Attach a provider manager for LLM-drafted skills.
    #[must_use]
    pub fn with_provider_manager(mut self, pm: Arc<ProviderManager>) -> Self {
        self.provider_manager = Some(pm);
        self
    }
}

#[async_trait]
impl ProjectSkillEvolver for ProjectSkillEvolverImpl {
    async fn evolve(&self, repo_id: &str) -> Result<Vec<SkillSynthesisResult>> {
        let candidates = detect_candidates(&self.pool, repo_id).await?;
        let mut results = Vec::new();

        for candidate in candidates {
            let skill_id = crate::skill_evolver::write::sanitize(&candidate.rule_text);

            // LLM-draft the SKILL.md body when a provider is available.
            let drafted_body = if let Some(ref pm) = self.provider_manager {
                draft_skill_md(pm, &candidate.rule_text).await.ok()
            } else {
                None
            };

            let (name, description, procedure) = if let Some(ref body) = drafted_body {
                parse_skill_frontmatter(body, &candidate.rule_text)
            } else {
                (
                    candidate.rule_text.clone(),
                    format!(
                        "Auto-detected workflow pattern (confidence {:.0}%)",
                        candidate.confidence * 100.0
                    ),
                    candidate.rule_text.clone(),
                )
            };

            let spec = ProjectSkillSpec {
                skill_id: skill_id.clone(),
                repo_id: repo_id.to_string(),
                name,
                description,
                when_to_use: vec![],
                procedure,
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

async fn draft_skill_md(pm: &ProviderManager, rule_text: &str) -> Result<String> {
    let prompt = format!(
        "Draft an Agent Skills SKILL.md (with YAML frontmatter: name, description, whenToUse) \
         for this observed workflow pattern. Use crisp procedural language.\n\nPattern:\n{}",
        rule_text
    );
    let messages = vec![providers::types::Message::User {
        content: providers::types::UserContent::Text(prompt),
    }];
    let params = providers::types::ChatParams::new("default".to_string());
    let resp = pm
        .chat_with_role(ProviderRole::ReforgeRules, &messages, None, &params)
        .await
        .map_err(|e| KlyntbotError::Storage(format!("skill draft: {e}")))?;
    Ok(resp.content.unwrap_or_else(|| rule_text.to_string()))
}

fn parse_skill_frontmatter(body: &str, fallback: &str) -> (String, String, String) {
    // Simple heuristic: split on first "---" to extract frontmatter.
    let mut lines = body.lines();
    let first = lines.next().unwrap_or("").trim();
    if first != "---" {
        return (
            fallback.to_string(),
            "Auto-detected workflow pattern".into(),
            body.to_string(),
        );
    }
    let mut front = String::new();
    let mut in_front = true;
    for line in lines {
        if in_front && line.trim() == "---" {
            in_front = false;
            continue;
        }
        if in_front {
            front.push_str(line);
            front.push('\n');
        }
    }
    let name = extract_yaml_field(&front, "name").unwrap_or_else(|| fallback.to_string());
    let description = extract_yaml_field(&front, "description")
        .unwrap_or_else(|| "Auto-detected workflow pattern".into());
    let procedure = body.to_string();
    (name, description, procedure)
}

fn extract_yaml_field(yaml: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}:");
    for line in yaml.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with(&prefix) {
            return Some(trimmed[prefix.len()..].trim().to_string());
        }
    }
    None
}
