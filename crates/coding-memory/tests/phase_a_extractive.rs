use coding_ingest::event::{
    AgentEvent, AgentEventV1, AgentSource, EventKind, FileOp, TokenUsage,
};
use coding_memory::distiller::phase_a::compute_turn_trace;
use jiff::Timestamp;
use std::path::PathBuf;
use uuid::Uuid;

fn wrap(kind: EventKind) -> AgentEvent {
    AgentEvent::V1(AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::ClaudeCode,
        session_id: "s".into(),
        turn_id: Some("t".into()),
        cwd: PathBuf::from("/tmp"),
        repo: None,
        occurred_at: Timestamp::now(),
        kind,
    })
}

#[test]
fn collects_file_edits_and_reads_separately() {
    let events = vec![
        wrap(EventKind::FileEdit {
            path: PathBuf::from("a.rs"), op: FileOp::Read, bytes: 100, diff_preview: None,
        }),
        wrap(EventKind::FileEdit {
            path: PathBuf::from("b.rs"), op: FileOp::Modify, bytes: 200, diff_preview: None,
        }),
        wrap(EventKind::FileEdit {
            path: PathBuf::from("c.rs"), op: FileOp::Create, bytes: 50, diff_preview: None,
        }),
    ];
    let trace = compute_turn_trace("s", Some("t"), &events);
    assert_eq!(trace.files_read.len(), 1);
    assert_eq!(trace.files_read[0], PathBuf::from("a.rs"));
    assert_eq!(trace.files_modified.len(), 2);
}

#[test]
fn captures_test_outcomes() {
    let events = vec![
        wrap(EventKind::TestRun {
            command: "cargo test".into(),
            framework: Some("cargo".into()),
            passed: 10, failed: 2, duration_ms: 1000,
        }),
    ];
    let trace = compute_turn_trace("s", Some("t"), &events);
    assert_eq!(trace.test_outcomes.len(), 1);
    assert_eq!(trace.test_outcomes[0].passed, 10);
    assert_eq!(trace.test_outcomes[0].failed, 2);
}

#[test]
fn captures_commands_from_bash_tool_calls() {
    let events = vec![
        wrap(EventKind::ToolCall {
            tool: "Bash".into(),
            args_preview: "cargo build".into(),
            ok: true, duration_ms: 500, result_preview: "ok".into(),
        }),
        wrap(EventKind::ToolCall {
            tool: "Read".into(), // non-bash tools ignored
            args_preview: "foo.rs".into(),
            ok: true, duration_ms: 1, result_preview: "".into(),
        }),
    ];
    let trace = compute_turn_trace("s", Some("t"), &events);
    assert_eq!(trace.commands_run.len(), 1);
    assert_eq!(trace.commands_run[0], "cargo build");
}

#[test]
fn captures_errors() {
    let events = vec![
        wrap(EventKind::Error { tool: Some("Bash".into()), message: "exit 1".into() }),
        wrap(EventKind::Error { tool: None, message: "generic".into() }),
    ];
    let trace = compute_turn_trace("s", Some("t"), &events);
    assert_eq!(trace.errors_encountered.len(), 2);
}

#[test]
fn final_assistant_msg_sets_token_usage() {
    let events = vec![
        wrap(EventKind::AssistantMsg {
            text: "partial".into(), truncated: true, token_usage: None,
        }),
        wrap(EventKind::AssistantMsg {
            text: "final".into(),
            truncated: false,
            token_usage: Some(TokenUsage { prompt_tokens: 100, completion_tokens: 50, cached_tokens: Some(25) }),
        }),
    ];
    let trace = compute_turn_trace("s", Some("t"), &events);
    let u = trace.token_usage.expect("usage set");
    assert_eq!(u.prompt, 100);
    assert_eq!(u.completion, 50);
    assert_eq!(u.cached, 25);
}
