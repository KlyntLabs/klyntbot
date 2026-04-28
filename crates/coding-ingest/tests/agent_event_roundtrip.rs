//! Invariant #7: `parse(serialize(AgentEvent)) == AgentEvent` for every
//! `EventKind` variant. Covers the 9 base + 10 klynt-cli rich variants.

use coding_ingest::event::{
    AgentEvent, AgentEventV1, AgentSource, DiagnosticsDelta, EventKind, FileOp, SkillScore,
    SymbolRef, TestFailure, TokenUsage,
};
use coding_ingest::scope::RepoScope;
use jiff::Timestamp;
use std::path::PathBuf;
use uuid::Uuid;

fn wrap(kind: EventKind) -> AgentEvent {
    AgentEvent::V1(AgentEventV1 {
        id: Uuid::nil(),
        source: AgentSource::ClaudeCode,
        session_id: "sess-1".into(),
        turn_id: Some("turn-1".into()),
        cwd: PathBuf::from("/tmp/repo"),
        repo: Some(RepoScope {
            repo_id: "github.com/user/repo".into(),
            root: PathBuf::from("/tmp/repo"),
            git_hash: Some("abc123".into()),
            branch: Some("main".into()),
        }),
        occurred_at: Timestamp::from_second(1_700_000_000).unwrap(),
        kind,
    })
}

fn assert_roundtrip(event: AgentEvent) {
    let json = serde_json::to_string(&event).expect("serialize");
    let back: AgentEvent = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(event, back, "roundtrip mismatch:\n{json}");
}

#[test]
fn session_start_roundtrip() {
    assert_roundtrip(wrap(EventKind::SessionStart {
        model: Some("claude-sonnet-4-6".into()),
        source_reason: "cli-arg".into(),
    }));
}

#[test]
fn session_end_roundtrip() {
    assert_roundtrip(wrap(EventKind::SessionEnd {
        reason: "user-quit".into(),
    }));
}

#[test]
fn user_prompt_roundtrip() {
    assert_roundtrip(wrap(EventKind::UserPrompt {
        text: "fix the test".into(),
        attachments: vec![PathBuf::from("/tmp/log.txt")],
    }));
}

#[test]
fn assistant_msg_roundtrip() {
    assert_roundtrip(wrap(EventKind::AssistantMsg {
        text: "done".into(),
        truncated: false,
        token_usage: Some(TokenUsage {
            prompt_tokens: 100,
            completion_tokens: 50,
            cached_tokens: Some(20),
        }),
    }));
}

#[test]
fn tool_call_roundtrip() {
    assert_roundtrip(wrap(EventKind::ToolCall {
        tool: "Bash".into(),
        args_preview: "ls -la".into(),
        ok: true,
        duration_ms: 12,
        result_preview: "...".into(),
    }));
}

#[test]
fn file_edit_roundtrip() {
    assert_roundtrip(wrap(EventKind::FileEdit {
        path: PathBuf::from("/tmp/main.rs"),
        op: FileOp::Modify,
        bytes: 1234,
        diff_preview: Some("@@ -1 +1 @@".into()),
    }));
}

#[test]
fn test_run_roundtrip() {
    assert_roundtrip(wrap(EventKind::TestRun {
        command: "cargo test".into(),
        framework: Some("cargo".into()),
        passed: 12,
        failed: 1,
        duration_ms: 8000,
    }));
}

#[test]
fn compact_event_roundtrip() {
    assert_roundtrip(wrap(EventKind::CompactEvent {
        trigger: "auto".into(),
        token_count: 32_000,
    }));
}

#[test]
fn error_roundtrip() {
    assert_roundtrip(wrap(EventKind::Error {
        tool: Some("Bash".into()),
        message: "command not found".into(),
    }));
}

#[test]
fn skill_activated_roundtrip() {
    assert_roundtrip(wrap(EventKind::SkillActivated {
        skill_id: "task-management".into(),
        source_path: PathBuf::from("/skills/task-management/SKILL.md"),
        trigger: "keyword:tasks".into(),
    }));
}

#[test]
fn recall_injected_roundtrip() {
    assert_roundtrip(wrap(EventKind::RecallInjected {
        memory_ids: vec!["ep_1".into(), "rc_2".into()],
        coverage_score: 0.78,
        dead_end_warning: true,
    }));
}

#[test]
fn approval_decision_roundtrip() {
    assert_roundtrip(wrap(EventKind::ApprovalDecision {
        tool: "Bash".into(),
        decision: "allow".into(),
        layer: "rules".into(),
    }));
}

#[test]
fn sandbox_applied_roundtrip() {
    assert_roundtrip(wrap(EventKind::SandboxApplied {
        tool: "Bash".into(),
        policy_summary: "no-net,fs-readonly".into(),
        fallback_unsandboxed: false,
    }));
}

#[test]
fn file_edit_enriched_roundtrip() {
    assert_roundtrip(wrap(EventKind::FileEditEnriched {
        path: PathBuf::from("/tmp/lib.rs"),
        op: FileOp::Create,
        anchored_symbols: vec![SymbolRef {
            file_path: PathBuf::from("/tmp/lib.rs"),
            symbol: "foo".into(),
            git_hash: "abc123".into(),
        }],
        lsp_diagnostics_delta: Some(DiagnosticsDelta {
            before: 3,
            after: 1,
            introduced: vec![],
            resolved: vec!["E0061".into(), "E0282".into()],
        }),
    }));
}

#[test]
fn test_run_enriched_roundtrip() {
    assert_roundtrip(wrap(EventKind::TestRunEnriched {
        command: "cargo nextest run".into(),
        passed_tests: vec!["foo::bar".into()],
        failed_tests: vec![TestFailure {
            test_name: "foo::baz".into(),
            message: "assertion failed".into(),
            stack: Some("at line 42".into()),
        }],
        newly_failing: vec!["foo::baz".into()],
    }));
}

#[test]
fn provider_call_roundtrip() {
    assert_roundtrip(wrap(EventKind::ProviderCall {
        model: "claude-haiku-4-5".into(),
        prompt_tokens: 1000,
        completion_tokens: 500,
        cost_usd: 0.0021,
        latency_ms: 850,
        retries: 0,
    }));
}

#[test]
fn compression_applied_roundtrip() {
    assert_roundtrip(wrap(EventKind::CompressionApplied {
        before_tokens: 80_000,
        after_tokens: 32_000,
        messages_condensed: 17,
    }));
}

#[test]
fn mirror_alert_roundtrip() {
    assert_roundtrip(wrap(EventKind::MirrorAlert {
        alert_id: "mir_42".into(),
        severity: "high".into(),
        kind: "ProblemClassRefactor".into(),
    }));
}

#[test]
fn skill_routing_trace_roundtrip() {
    assert_roundtrip(wrap(EventKind::SkillRoutingTrace {
        considered: vec![SkillScore {
            skill_id: "task-management".into(),
            score: 0.91,
            components: vec![("keyword".into(), 0.6), ("semantic".into(), 0.31)],
        }],
        chosen: vec!["task-management".into()],
    }));
}

#[test]
fn no_repo_no_turn_roundtrip() {
    let event = AgentEvent::V1(AgentEventV1 {
        id: Uuid::nil(),
        source: AgentSource::Codex,
        session_id: "s".into(),
        turn_id: None,
        cwd: PathBuf::from("/x"),
        repo: None,
        occurred_at: Timestamp::from_second(0).unwrap(),
        kind: EventKind::SessionEnd { reason: "x".into() },
    });
    assert_roundtrip(event);
}

fn wrap_with_source(source: AgentSource, kind: EventKind) -> AgentEvent {
    AgentEvent::V1(AgentEventV1 {
        id: Uuid::nil(),
        source,
        session_id: "sess-x".into(),
        turn_id: Some("turn-x".into()),
        cwd: PathBuf::from("/tmp/repo"),
        repo: None,
        occurred_at: Timestamp::from_second(1_700_000_000).unwrap(),
        kind,
    })
}

#[test]
fn kimi_cli_source_roundtrip() {
    assert_roundtrip(wrap_with_source(
        AgentSource::KimiCli,
        EventKind::SessionStart {
            model: Some("kimi-k1.5".into()),
            source_reason: "hook".into(),
        },
    ));
    assert_roundtrip(wrap_with_source(
        AgentSource::KimiCli,
        EventKind::ToolCall {
            tool: "shell".into(),
            args_preview: "ls".into(),
            ok: true,
            duration_ms: 5,
            result_preview: "...".into(),
        },
    ));
}

#[test]
fn opencode_source_roundtrip() {
    assert_roundtrip(wrap_with_source(
        AgentSource::OpenCode,
        EventKind::UserPrompt {
            text: "fix it".into(),
            attachments: vec![],
        },
    ));
    assert_roundtrip(wrap_with_source(
        AgentSource::OpenCode,
        EventKind::AssistantMsg {
            text: "ok".into(),
            truncated: false,
            token_usage: None,
        },
    ));
}

#[test]
fn klynt_cli_source_roundtrip() {
    assert_roundtrip(wrap_with_source(
        AgentSource::KlyntCli,
        EventKind::SkillRoutingTrace {
            considered: vec![SkillScore {
                skill_id: "task-management".into(),
                score: 0.5,
                components: vec![("kw".into(), 0.5)],
            }],
            chosen: vec!["task-management".into()],
        },
    ));
    assert_roundtrip(wrap_with_source(
        AgentSource::KlyntCli,
        EventKind::CompressionApplied {
            before_tokens: 10_000,
            after_tokens: 4_000,
            messages_condensed: 3,
        },
    ));
}
