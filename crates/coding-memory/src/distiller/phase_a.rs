//! Phase A — deterministic extractive pass.
//!
//! Runs before any LLM call. Produces a `TurnTrace` covering:
//! - Files read vs. modified (with byte deltas for modifications)
//! - Shell commands (Bash tool calls only)
//! - Test-run outcomes
//! - Errors encountered
//! - Token usage from the *final* `AssistantMsg` that carried usage
//!
//! Never reads the LLM, never fails. Output feeds both:
//! 1. The Phase-B prompt (compact structured summary).
//! 2. A durable `episodic_memories { kind: 'turn_trace' }` row (Task 10).

use super::{TestOutcome, TurnTokenUsage, TurnTrace};
use coding_ingest::event::{AgentEvent, EventKind, FileOp};
use jiff::Timestamp;

/// Build the `TurnTrace` for one turn from its ordered events.
pub fn compute_turn_trace(
    session_id: &str,
    turn_id: Option<&str>,
    events: &[AgentEvent],
) -> TurnTrace {
    let mut files_read = Vec::new();
    let mut files_modified: Vec<(std::path::PathBuf, i64)> = Vec::new();
    let mut commands_run = Vec::new();
    let mut test_outcomes = Vec::new();
    let mut errors_encountered = Vec::new();
    let mut token_usage: Option<TurnTokenUsage> = None;
    let mut started_at: Option<Timestamp> = None;
    let mut ended_at: Option<Timestamp> = None;

    for event in events {
        let AgentEvent::V1(v1) = event;
        started_at.get_or_insert(v1.occurred_at);
        ended_at = Some(v1.occurred_at);

        match &v1.kind {
            EventKind::FileEdit {
                path, op, bytes, ..
            } => match op {
                FileOp::Read => files_read.push(path.clone()),
                FileOp::Create | FileOp::Modify | FileOp::Delete => {
                    files_modified.push((path.clone(), *bytes as i64));
                }
            },
            EventKind::FileEditEnriched { path, op, .. } => match op {
                FileOp::Read => files_read.push(path.clone()),
                _ => files_modified.push((path.clone(), 0)),
            },
            EventKind::ToolCall {
                tool, args_preview, ..
            } if tool.eq_ignore_ascii_case("bash") => {
                commands_run.push(args_preview.clone());
            }
            EventKind::TestRun {
                command,
                framework,
                passed,
                failed,
                ..
            } => {
                test_outcomes.push(TestOutcome {
                    command: command.clone(),
                    framework: framework.clone(),
                    passed: *passed,
                    failed: *failed,
                });
            }
            EventKind::TestRunEnriched {
                command,
                passed_tests,
                failed_tests,
                ..
            } => {
                test_outcomes.push(TestOutcome {
                    command: command.clone(),
                    framework: None,
                    passed: passed_tests.len() as u32,
                    failed: failed_tests.len() as u32,
                });
            }
            EventKind::Error { tool, message } => {
                errors_encountered.push((tool.clone(), message.clone()));
            }
            EventKind::AssistantMsg {
                token_usage: Some(u),
                ..
            } => {
                token_usage = Some(TurnTokenUsage {
                    prompt: u.prompt_tokens,
                    completion: u.completion_tokens,
                    cached: u.cached_tokens.unwrap_or(0),
                });
            }
            _ => {}
        }
    }

    TurnTrace {
        session_id: session_id.to_string(),
        turn_id: turn_id.map(str::to_string),
        files_read,
        files_modified,
        commands_run,
        test_outcomes,
        errors_encountered,
        token_usage,
        started_at: started_at.unwrap_or_else(Timestamp::now),
        ended_at,
    }
}

use super::error::DistillerError;
use super::writer::{DistillerWriter, PreparedEpisode};
use crate::scope::ProvenanceMetadata;
use cognitive::types::EpisodicMemory;
use uuid::Uuid;

/// Persist a `TurnTrace` as an `episodic_memories { kind: 'turn_trace' }` row
/// through the provenance-enforcing `DistillerWriter`. Returns the new row's id.
pub async fn persist_turn_trace(
    writer: &DistillerWriter,
    trace: &TurnTrace,
    scope_repo_id: Option<&str>,
    provenance: &ProvenanceMetadata,
) -> Result<Uuid, DistillerError> {
    let id = Uuid::new_v4();
    let content = serde_json::json!({
        "filesRead": trace.files_read.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>(),
        "filesModified": trace.files_modified.iter()
            .map(|(p, n)| serde_json::json!({"path": p.to_string_lossy(), "bytes": n}))
            .collect::<Vec<_>>(),
        "commandsRun": trace.commands_run,
        "testOutcomes": trace.test_outcomes.iter().map(|t| serde_json::json!({
            "command": t.command,
            "framework": t.framework,
            "passed": t.passed,
            "failed": t.failed,
        })).collect::<Vec<_>>(),
        "errorsEncountered": trace.errors_encountered,
        "tokenUsage": trace.token_usage.map(|u| serde_json::json!({
            "prompt": u.prompt, "completion": u.completion, "cached": u.cached
        })),
    })
    .to_string();

    let importance = importance_for_trace(trace);
    let episode = EpisodicMemory {
        id: id.to_string(),
        domain: "coding".into(),
        content,
        summary: None,
        importance,
        occurred_at: trace.started_at.to_string(),
        recorded_at: jiff::Timestamp::now().to_string(),
        stability: 1.0,
        last_accessed: None,
        access_count: 0,
        project_id: None,
        scope_type: if scope_repo_id.is_some() {
            "project".into()
        } else {
            "user".into()
        },
        scope_id: scope_repo_id.map(str::to_string),
    };

    writer
        .write_episode(PreparedEpisode {
            episode,
            kind: "turn_trace".into(),
            metadata_json: None,
            scope_repo_id: scope_repo_id.map(str::to_string),
            provenance: provenance.clone(),
        })
        .await?;
    Ok(id)
}

fn importance_for_trace(t: &TurnTrace) -> f64 {
    let mut score: f64 = 0.3;
    if !t.files_modified.is_empty() {
        score += 0.2;
    }
    if t.test_outcomes.iter().any(|x| x.failed > 0) {
        score += 0.2;
    }
    if !t.errors_encountered.is_empty() {
        score += 0.2;
    }
    score.min(1.0)
}
