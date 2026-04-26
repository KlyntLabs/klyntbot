//! Phase 2.5 — Coding Synthesis.
//!
//! Pulls inputs, runs `CodingSynthesisHandler`, then applies each
//! `PromoteAction` to the cognitive store. Every write goes through the
//! existing `SemanticFactRepo` / `EpisodicMemoryRepo` / `ProceduralRuleRepo`
//! to inherit provenance, bi-temporal lifecycle, and FSRS5 init.

use crate::mirror::alerts::{CodingMirrorAlertKind, MirrorAlertSeverity};
use crate::reforge::types::{
    CausalChainGroup, CodingPhaseHandlers, CodingSynthesisInput, PromoteAction,
    RepoSynthesisBundle, SerializableEpisodicMemory, SerializableProceduralRule,
    SerializableSemanticFact,
};
use cognitive::types::{ProceduralRule, SemanticFact, DEFAULT_MEMORY_TYPE};
use common::{KlyntbotError, Result};
use jiff::Timestamp;
use serde_json::json;
use tracing::{info, warn};
use uuid::Uuid;

/// Orchestrator for Phase 2.5.
#[derive(Debug)]
pub struct CodingSynthesisPhase;

impl CodingSynthesisPhase {
    /// Run the phase. Returns the number of actions persisted.
    pub async fn run(handlers: &CodingPhaseHandlers<'_>) -> Result<u32> {
        let Some(handler) = handlers.synthesis else {
            return Ok(0);
        };
        let input = build_input(handlers).await?;
        let output = match handler.synthesize_coding(&input).await {
            Ok(o) => o,
            Err(e) => {
                warn!("Phase 2.5 LLM call failed: {e}");
                return Err(e);
            }
        };

        let mut applied = 0_u32;
        for action in output.actions {
            match apply_action(action, handlers).await {
                Ok(()) => applied += 1,
                Err(e) => warn!("Phase 2.5 apply failed: {e}"),
            }
        }
        info!(applied, narrative = %output.narrative, "Phase 2.5 complete");
        Ok(applied)
    }
}

async fn build_input(handlers: &CodingPhaseHandlers<'_>) -> Result<CodingSynthesisInput> {
    // Window: facts/episodes since 24h ago. Real cron schedule is nightly.
    let since = Timestamp::now()
        .checked_sub(jiff::SignedDuration::from_hours(24))
        .unwrap_or_else(|_| Timestamp::now());

    // Group fix-attempts and workflow patterns by repo.
    let pool = handlers.session_summary_repo.pool().clone();
    let repo_ids: Vec<(Option<String>,)> = sqlx::query_as::<_, (Option<String>,)>(
        "SELECT DISTINCT scope_repo_id FROM episodic_memories \
         WHERE recorded_at >= ?1 AND kind IN ('fix_attempt','test_run','refactor')",
    )
    .bind(since.to_string())
    .fetch_all(pool.inner())
    .await
    .map_err(|e| KlyntbotError::Storage(format!("repo list: {e}")))?;

    let mut bundles = Vec::new();
    for (repo_id,) in repo_ids {
        let Some(rid) = repo_id else { continue };
        let fix_attempts = fetch_fix_attempts(&pool, &rid, &since).await?;
        let patterns = fetch_workflow_patterns(&pool, &rid).await?;
        let context_facts = fetch_repo_context(&pool, &rid).await?;
        let causal_chains = fetch_causal_chain_groups(&pool, &rid).await?;
        bundles.push(RepoSynthesisBundle {
            repo_id: rid,
            fix_attempts,
            workflow_patterns: patterns,
            repo_context_facts: context_facts,
            causal_chains,
        });
    }

    let recent_counterfactuals = fetch_counterfactuals(&pool, &since).await?;
    Ok(CodingSynthesisInput {
        since,
        repo_bundles: bundles,
        recent_counterfactuals,
    })
}

async fn fetch_fix_attempts(
    pool: &storage::StoragePool,
    repo_id: &str,
    since: &Timestamp,
) -> Result<Vec<SerializableEpisodicMemory>> {
    let rows: Vec<(String, String, f32, String, Option<String>)> = sqlx::query_as(
        "SELECT id, content, importance, recorded_at, scope_repo_id \
         FROM episodic_memories \
         WHERE scope_repo_id = ?1 AND kind = 'fix_attempt' AND recorded_at >= ?2 \
         ORDER BY recorded_at DESC LIMIT 50",
    )
    .bind(repo_id)
    .bind(since.to_string())
    .fetch_all(pool.inner())
    .await
    .map_err(|e| KlyntbotError::Storage(format!("fix attempts: {e}")))?;
    Ok(rows
        .into_iter()
        .map(|(id, content, importance, recorded_at, scope_repo_id)| {
            SerializableEpisodicMemory {
                id,
                kind: "fix_attempt".into(),
                content,
                importance,
                recorded_at,
                scope_repo_id,
            }
        })
        .collect())
}

async fn fetch_workflow_patterns(
    pool: &storage::StoragePool,
    repo_id: &str,
) -> Result<Vec<SerializableProceduralRule>> {
    let rows: Vec<(String, String, String, f32, f32, Option<String>)> = sqlx::query_as(
        "SELECT id, rule_text, source, confidence, COALESCE(effectiveness_score, 0.5), scope_repo_id \
         FROM procedural_rules \
         WHERE scope_repo_id = ?1 AND source = 'observed'",
    )
    .bind(repo_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| KlyntbotError::Storage(format!("workflow patterns: {e}")))?;
    Ok(rows
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
        .collect())
}

#[allow(clippy::type_complexity)]
async fn fetch_repo_context(
    pool: &storage::StoragePool,
    repo_id: &str,
) -> Result<Vec<SerializableSemanticFact>> {
    let rows: Vec<(
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
                COALESCE(memory_type, 'fact'), \
                COALESCE(json_extract(metadata, '$.sensitivity'), 'normal'), \
                scope_repo_id, valid_from \
         FROM semantic_facts \
         WHERE scope_repo_id = ?1 AND domain = 'work' \
         ORDER BY confidence DESC LIMIT 50",
    )
    .bind(repo_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| KlyntbotError::Storage(format!("repo context: {e}")))?;
    Ok(rows
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
        .collect())
}

async fn fetch_causal_chain_groups(
    pool: &storage::StoragePool,
    repo_id: &str,
) -> Result<Vec<CausalChainGroup>> {
    // Phase 6 will populate edges with problem_hash. Phase 5 returns empty groups.
    let _ = pool;
    let _ = repo_id;
    Ok(Vec::new())
}

#[allow(clippy::type_complexity)]
async fn fetch_counterfactuals(
    pool: &storage::StoragePool,
    since: &Timestamp,
) -> Result<Vec<SerializableSemanticFact>> {
    let rows: Vec<(
        String,
        String,
        String,
        String,
        f32,
        String,
        Option<String>,
        String,
    )> = sqlx::query_as(
        "SELECT id, subject, predicate, object, confidence, \
                COALESCE(json_extract(metadata, '$.sensitivity'), 'normal'), \
                scope_repo_id, valid_from \
         FROM semantic_facts \
         WHERE memory_type = 'counterfactual' AND valid_from >= ?1 \
         ORDER BY confidence DESC LIMIT 100",
    )
    .bind(since.to_string())
    .fetch_all(pool.inner())
    .await
    .map_err(|e| KlyntbotError::Storage(format!("counterfactuals: {e}")))?;
    Ok(rows
        .into_iter()
        .map(
            |(id, subject, predicate, object, confidence, sensitivity, scope_repo_id, valid_from)| {
                SerializableSemanticFact {
                    id,
                    subject,
                    predicate,
                    object,
                    confidence,
                    memory_type: "counterfactual".into(),
                    sensitivity,
                    scope_repo_id,
                    valid_from,
                }
            },
        )
        .collect())
}

async fn apply_action(action: PromoteAction, handlers: &CodingPhaseHandlers<'_>) -> Result<()> {
    match action {
        PromoteAction::ExtractPattern { repo_id, rule, confidence, supporting } => {
            let r = ProceduralRule {
                id: format!("rule_{}", Uuid::new_v4().simple()),
                domain: "code".into(),
                rule_text: rule,
                source: "observed".into(),
                confidence: confidence as f64,
                signal_count: 1,
                created_at: Timestamp::now().to_string(),
                updated_at: Timestamp::now().to_string(),
                active: true,
                project_id: None,
                scope_type: "code".into(),
                scope_id: None,
                effectiveness_score: 0.5,
                stability: 1.0,
                scope_repo_id: repo_id.clone(),
                last_applied: None,
                application_count: 0,
                metadata: Some(json!({
                    "provenance": {
                        "source": "reforge.coding_synthesis",
                        "supporting": supporting,
                    },
                    "kind": "workflow_pattern",
                }).to_string()),
            };
            handlers.rule_repo.insert(&r).await?;
        }
        PromoteAction::ExtractFailurePattern { repo_id, rule, remediation, confidence, supporting } => {
            let combined = format!("{rule}\n\n**Remediation:** {remediation}");
            let r = ProceduralRule {
                id: format!("rule_{}", Uuid::new_v4().simple()),
                domain: "code".into(),
                rule_text: combined,
                source: "observed".into(),
                confidence: confidence as f64,
                signal_count: 1,
                created_at: Timestamp::now().to_string(),
                updated_at: Timestamp::now().to_string(),
                active: true,
                project_id: None,
                scope_type: "code".into(),
                scope_id: None,
                effectiveness_score: 0.5,
                stability: 1.0,
                scope_repo_id: repo_id.clone(),
                last_applied: None,
                application_count: 0,
                metadata: Some(json!({
                    "provenance": {
                        "source": "reforge.coding_synthesis",
                        "supporting": supporting,
                    },
                    "kind": "failure_pattern",
                }).to_string()),
            };
            handlers.rule_repo.insert(&r).await?;
        }
        PromoteAction::PromoteToProblemClass { problem_hash, suggestion } => {
            // No fact write — emit a Mirror alert via the bus if available.
            if let Some(bus) = handlers.bus.clone() {
                let alert_kind = CodingMirrorAlertKind::ProblemClassRefactor;
                let payload = json!({
                    "problem_hash": problem_hash,
                    "suggestion": suggestion,
                });
                bus.publish(bus::DomainEvent::CodingMirrorAlert {
                    kind: alert_kind.as_str().to_string(),
                    severity: MirrorAlertSeverity::High.as_str().to_string(),
                    payload: payload.to_string(),
                });
            }
        }
        PromoteAction::PromoteToProjectUnderstanding { repo_id, subject, predicate, object, convergence } => {
            if convergence < 0.7 {
                return Ok(());
            }
            let f = SemanticFact {
                id: format!("fact_{}", Uuid::new_v4().simple()),
                subject,
                predicate,
                object,
                confidence: convergence as f64,
                source: "reforge.coding_synthesis".into(),
                memory_type: DEFAULT_MEMORY_TYPE.to_string(),
                domain: "work".into(),
                scope_type: "code".into(),
                scope_id: None,
                scope_repo_id: Some(repo_id),
                valid_from: Timestamp::now().to_string(),
                valid_until: None,
                recorded_at: Timestamp::now().to_string(),
                superseded_at: None,
                superseded_by: None,
                stability: 2.0,
                last_accessed: None,
                access_count: 0,
                convergence_score: 0.0,
                project_id: None,
                metadata: Some(json!({
                    "provenance": {
                        "source": "reforge.coding_synthesis",
                        "kind": "project_understanding",
                    },
                    "convergence_score": convergence,
                }).to_string()),
            };
            handlers.fact_repo.upsert(&f).await?;
        }
        PromoteAction::PromoteToUserHabit { rule, confidence, witness_repos } => {
            let r = ProceduralRule {
                id: format!("rule_{}", Uuid::new_v4().simple()),
                domain: "code".into(),
                rule_text: rule,
                source: "reflected".into(),
                confidence: confidence as f64,
                signal_count: 1,
                created_at: Timestamp::now().to_string(),
                updated_at: Timestamp::now().to_string(),
                active: true,
                project_id: None,
                scope_type: "user".into(),
                scope_id: None,
                effectiveness_score: 0.5,
                stability: 1.5,
                scope_repo_id: None,
                last_applied: None,
                application_count: 0,
                metadata: Some(json!({
                    "provenance": {
                        "source": "reforge.coding_synthesis",
                        "kind": "user_habit",
                        "witness_repos": witness_repos,
                    },
                }).to_string()),
            };
            handlers.rule_repo.insert(&r).await?;
        }
        PromoteAction::PromoteToProblemSolutionPattern { problem_hash, solution, supporting_edges } => {
            let r = ProceduralRule {
                id: format!("rule_{}", Uuid::new_v4().simple()),
                domain: "code".into(),
                rule_text: format!("When problem matches `{problem_hash}`: {solution}"),
                source: "reflected".into(),
                confidence: 0.8f64,
                signal_count: 1,
                created_at: Timestamp::now().to_string(),
                updated_at: Timestamp::now().to_string(),
                active: true,
                project_id: None,
                scope_type: "code".into(),
                scope_id: None,
                effectiveness_score: 0.5,
                stability: 1.5,
                scope_repo_id: None,
                last_applied: None,
                application_count: 0,
                metadata: Some(json!({
                    "provenance": {
                        "source": "reforge.coding_synthesis",
                        "kind": "problem_solution_pattern",
                        "supporting_edges": supporting_edges,
                    },
                    "problem_hash": problem_hash,
                }).to_string()),
            };
            handlers.rule_repo.insert(&r).await?;
        }
    }
    Ok(())
}
