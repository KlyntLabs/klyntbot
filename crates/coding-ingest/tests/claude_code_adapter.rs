use coding_ingest::adapters::claude_code::ClaudeCodeAdapter;
use coding_ingest::adapters::IngestAdapter;
use coding_ingest::event::{AgentEvent, EventKind};

fn parse(event: &str, body: &str) -> Option<AgentEvent> {
    ClaudeCodeAdapter.parse(event, body.as_bytes()).unwrap()
}

#[test]
fn session_start() {
    let body = r#"{
        "session_id": "abc",
        "cwd": "/tmp/repo",
        "source": "cli",
        "model": "claude-sonnet-4-6"
    }"#;
    let AgentEvent::V1(v1) = parse("SessionStart", body).unwrap();
    assert_eq!(v1.session_id, "abc");
    matches!(v1.kind, EventKind::SessionStart { .. });
}

#[test]
fn user_prompt() {
    let body = r#"{
        "session_id": "s", "cwd": "/tmp",
        "prompt": "hello",
        "attachments": ["/tmp/a.png"]
    }"#;
    let AgentEvent::V1(v1) = parse("UserPromptSubmit", body).unwrap();
    match v1.kind {
        EventKind::UserPrompt { text, attachments } => {
            assert_eq!(text, "hello");
            assert_eq!(attachments.len(), 1);
        }
        _ => panic!("wrong kind"),
    }
}

#[test]
fn stop_becomes_assistant_msg() {
    let body = r#"{
        "session_id": "s", "cwd": "/tmp",
        "transcript_path": "/tmp/tr.jsonl",
        "stop_hook_active": false
    }"#;
    let AgentEvent::V1(v1) = parse("Stop", body).unwrap();
    matches!(v1.kind, EventKind::AssistantMsg { .. });
}

#[test]
fn session_end() {
    let body = r#"{"session_id": "s", "cwd": "/tmp", "reason": "user-quit"}"#;
    let AgentEvent::V1(v1) = parse("SessionEnd", body).unwrap();
    match v1.kind {
        EventKind::SessionEnd { reason } => assert_eq!(reason, "user-quit"),
        _ => panic!(),
    }
}

#[test]
fn pre_compact_becomes_compact_event() {
    let body =
        r#"{"session_id": "s", "cwd": "/tmp", "trigger": "auto", "custom_instructions": ""}"#;
    let AgentEvent::V1(v1) = parse("PreCompact", body).unwrap();
    matches!(v1.kind, EventKind::CompactEvent { .. });
}

#[test]
fn unknown_hook_returns_none() {
    let body = r#"{}"#;
    assert!(parse("Unknown", body).is_none());
}

#[test]
fn post_tool_use_bash_test_becomes_test_run() {
    let body = r#"{
        "session_id": "s", "cwd": "/tmp",
        "tool_name": "Bash",
        "tool_input": {"command": "cargo test --workspace"},
        "tool_response": {
            "stdout": "test result: ok. 12 passed; 3 failed; 0 ignored",
            "stderr": "", "exit_code": 1
        },
        "duration_ms": 4200
    }"#;
    let AgentEvent::V1(v1) = parse("PostToolUse", body).unwrap();
    match v1.kind {
        EventKind::TestRun {
            framework,
            passed,
            failed,
            duration_ms,
            ..
        } => {
            assert_eq!(framework.as_deref(), Some("cargo"));
            assert_eq!(passed, 12);
            assert_eq!(failed, 3);
            assert_eq!(duration_ms, 4200);
        }
        other => panic!("expected TestRun, got {other:?}"),
    }
}

#[test]
fn post_tool_use_bash_non_test_becomes_tool_call() {
    let body = r#"{
        "session_id": "s", "cwd": "/tmp",
        "tool_name": "Bash",
        "tool_input": {"command": "ls -la"},
        "tool_response": {"stdout": "...", "stderr": "", "exit_code": 0},
        "duration_ms": 15
    }"#;
    let AgentEvent::V1(v1) = parse("PostToolUse", body).unwrap();
    matches!(v1.kind, EventKind::ToolCall { .. });
}

#[test]
fn post_tool_use_edit_becomes_file_edit() {
    let body = r#"{
        "session_id": "s", "cwd": "/tmp",
        "tool_name": "Edit",
        "tool_input": {"file_path": "/tmp/src/main.rs", "old_string": "a", "new_string": "b"},
        "tool_response": {"success": true, "bytes": 1234},
        "duration_ms": 8
    }"#;
    let AgentEvent::V1(v1) = parse("PostToolUse", body).unwrap();
    match v1.kind {
        EventKind::FileEdit { path, bytes, .. } => {
            assert_eq!(path.to_string_lossy(), "/tmp/src/main.rs");
            assert_eq!(bytes, 1234);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn post_tool_use_write_is_create_file_op() {
    let body = r#"{
        "session_id": "s", "cwd": "/tmp",
        "tool_name": "Write",
        "tool_input": {"file_path": "/tmp/new.rs", "content": "fn main(){}"},
        "tool_response": {"bytes": 11},
        "duration_ms": 3
    }"#;
    let AgentEvent::V1(v1) = parse("PostToolUse", body).unwrap();
    match v1.kind {
        EventKind::FileEdit { op, .. } => assert_eq!(op, coding_ingest::event::FileOp::Create),
        other => panic!("{other:?}"),
    }
}

#[test]
fn post_tool_use_read_becomes_file_edit_read_op() {
    let body = r#"{
        "session_id": "s", "cwd": "/tmp",
        "tool_name": "Read",
        "tool_input": {"file_path": "/tmp/x.rs"},
        "tool_response": {"bytes": 500},
        "duration_ms": 2
    }"#;
    let AgentEvent::V1(v1) = parse("PostToolUse", body).unwrap();
    match v1.kind {
        EventKind::FileEdit { op, .. } => assert_eq!(op, coding_ingest::event::FileOp::Read),
        other => panic!("{other:?}"),
    }
}

#[test]
fn post_tool_use_pytest_framework_detected() {
    let body = r#"{
        "session_id": "s", "cwd": "/tmp",
        "tool_name": "Bash",
        "tool_input": {"command": "pytest -v"},
        "tool_response": {
            "stdout": "==== 5 passed, 1 failed, 2 skipped in 0.23s ====",
            "stderr": "", "exit_code": 1
        },
        "duration_ms": 230
    }"#;
    let AgentEvent::V1(v1) = parse("PostToolUse", body).unwrap();
    match v1.kind {
        EventKind::TestRun {
            framework,
            passed,
            failed,
            ..
        } => {
            assert_eq!(framework.as_deref(), Some("pytest"));
            assert_eq!(passed, 5);
            assert_eq!(failed, 1);
        }
        other => panic!("{other:?}"),
    }
}
