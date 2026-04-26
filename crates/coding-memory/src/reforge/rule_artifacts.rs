//! Phase 3.5 — Rule Artifact Generation.
//!
//! Per repo: filter facts by sensitivity, build a `RepoArtifactPlan`, call the
//! handler once per enabled artifact kind, write each result via `ManagedBlock`
//! atomically. Failure isolation: a failed artifact does not abort the others.

use crate::reforge::managed_block::{ManagedBlock, ManagedBlockError};
use crate::reforge::sensitivity_filter::filter_for_externalization;
use crate::reforge::types::{
    CodingPhaseHandlers, RepoArtifactPlan, RuleArtifactInput, SerializableProceduralRule,
    SerializableSemanticFact,
};
use crate::reforge_phase::RuleArtifact;
use common::{KlyntbotError, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{info, warn};
use uuid::Uuid;

/// Orchestrator for Phase 3.5.
#[derive(Debug)]
pub struct RuleArtifactGenerationPhase;

impl RuleArtifactGenerationPhase {
    /// Run the phase.
    ///
    /// `enabled_artifacts` is the configured allowlist
    /// (`config.codingMemory.reforge.ruleArtifacts.*`). Strings: `claude_md`,
    /// `agents_md`, `cursorrules`, `continue_rules`.
    pub async fn run(
        handlers: &CodingPhaseHandlers<'_>,
        enabled_artifacts: &[String],
    ) -> Result<u32> {
        let Some(handler) = handlers.rule_artifacts else {
            return Ok(0);
        };
        let cycle_id = format!("c-{}", Uuid::new_v4().simple());
        let mut artifacts_written = 0_u32;

        // 1. Discover repos with both a known on-disk path and at least one
        //    repo-scoped fact.
        let repo_paths = load_repo_paths()?;
        let pool = handlers.session_summary_repo.pool().clone();
        let repo_ids: Vec<(String,)> = sqlx::query_as::<_, (String,)>(
            "SELECT DISTINCT scope_repo_id FROM semantic_facts \
             WHERE scope_repo_id IS NOT NULL",
        )
        .fetch_all(pool.inner())
        .await
        .map_err(|e| KlyntbotError::Storage(format!("repo enum: {e}")))?;

        for (repo_id,) in repo_ids {
            let Some(root) = repo_paths.get(&repo_id) else {
                continue;
            };
            let plan = match build_plan(handlers, &repo_id, root.clone(), enabled_artifacts).await {
                Ok(p) => p,
                Err(e) => {
                    warn!("Phase 3.5 plan build failed for {repo_id}: {e}");
                    continue;
                }
            };

            for artifact in &plan.enabled {
                let input = RuleArtifactInput {
                    plan: plan.clone(),
                    artifact: *artifact,
                };
                let output = match handler.synthesize_artifact(&input).await {
                    Ok(o) => o,
                    Err(e) => {
                        warn!("Phase 3.5 LLM call failed for {:?}: {e}", artifact);
                        continue;
                    }
                };
                let path = root.join(artifact.relative_path());
                let block = ManagedBlock::read(&path).unwrap_or_default();
                match block.write_with_new_inside(&path, &output.body, &cycle_id) {
                    Ok(()) => {
                        artifacts_written += 1;
                        info!("Phase 3.5 wrote {} for {repo_id}", path.display());
                    }
                    Err(ManagedBlockError::UserConflict) => {
                        warn!("Phase 3.5 user conflict on {}; skipping", path.display());
                    }
                    Err(e) => warn!("Phase 3.5 write failed for {}: {e}", path.display()),
                }
            }
        }
        Ok(artifacts_written)
    }
}

#[allow(clippy::type_complexity)]
async fn build_plan(
    handlers: &CodingPhaseHandlers<'_>,
    repo_id: &str,
    root: PathBuf,
    enabled_artifacts: &[String],
) -> Result<RepoArtifactPlan> {
    let pool = handlers.session_summary_repo.pool().clone();
    let fact_rows: Vec<(
        String,
        String,
        String,
        String,
        f32,
        String,
        String,
        Option<String>,
        String,
    )> = sqlx::query_as(
        "SELECT id, subject, predicate, object, confidence, \
                COALESCE(memory_type,'fact'), \
                COALESCE(json_extract(metadata, '$.sensitivity'),'normal'), \
                scope_repo_id, valid_from \
         FROM semantic_facts \
         WHERE scope_repo_id = ?1 AND confidence >= 0.7",
    )
    .bind(repo_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| KlyntbotError::Storage(format!("plan facts: {e}")))?;
    let facts: Vec<SerializableSemanticFact> = fact_rows
        .into_iter()
        .map(
            |(id, subject, predicate, object, confidence, memory_type, sensitivity, scope_repo_id, valid_from)| {
                SerializableSemanticFact {
                    id,
                    subject,
                    predicate,
                    object,
                    confidence,
                    memory_type,
                    sensitivity,
                    scope_repo_id,
                    valid_from,
                }
            },
        )
        .collect();
    let facts = filter_for_externalization(&facts);

    let rule_rows: Vec<(String, String, String, f32, f32, Option<String>)> = sqlx::query_as(
        "SELECT id, rule_text, source, confidence, COALESCE(effectiveness_score, 0.5), scope_repo_id \
         FROM procedural_rules \
         WHERE scope_repo_id = ?1 AND confidence >= 0.7 AND \
               COALESCE(effectiveness_score, 0.5) >= 0.5",
    )
    .bind(repo_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| KlyntbotError::Storage(format!("plan rules: {e}")))?;
    let rules: Vec<SerializableProceduralRule> = rule_rows
        .into_iter()
        .map(|(id, rule, source, confidence, effectiveness, scope_repo_id)| {
            SerializableProceduralRule {
                id,
                rule,
                source,
                confidence,
                effectiveness,
                scope_repo_id,
            }
        })
        .collect();

    let enabled = enabled_artifacts
        .iter()
        .filter_map(|s| match s.as_str() {
            "claude_md" => Some(RuleArtifact::ClaudeMd),
            "agents_md" => Some(RuleArtifact::AgentsMd),
            "cursorrules" => Some(RuleArtifact::CursorRules),
            "continue_rules" => Some(RuleArtifact::ContinueRules),
            _ => None,
        })
        .collect();

    Ok(RepoArtifactPlan {
        repo_id: repo_id.to_string(),
        root,
        enabled,
        facts,
        rules,
    })
}

fn load_repo_paths() -> Result<HashMap<String, PathBuf>> {
    if let Ok(json) = std::env::var("KLYNTBOT_REPO_PATHS_TEST_OVERRIDE") {
        return serde_json::from_str::<HashMap<String, String>>(&json)
            .map(|m| m.into_iter().map(|(k, v)| (k, PathBuf::from(v))).collect())
            .map_err(|e| KlyntbotError::Storage(format!("repo paths test override: {e}")));
    }
    let home = dirs::home_dir().ok_or_else(|| KlyntbotError::Storage("no home dir".into()))?;
    let path = home.join(".klyntbot").join("repo_paths.json");
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| KlyntbotError::Storage(format!("repo_paths.json read: {e}")))?;
    let map: HashMap<String, String> = serde_json::from_str(&raw)
        .map_err(|e| KlyntbotError::Storage(format!("repo_paths.json parse: {e}")))?;
    Ok(map.into_iter().map(|(k, v)| (k, PathBuf::from(v))).collect())
}
