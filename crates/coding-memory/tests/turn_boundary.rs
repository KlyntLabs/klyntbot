use coding_ingest::event::{AgentEvent, AgentEventV1, AgentSource, EventKind, TokenUsage};
use coding_memory::distiller::turn_buffer::{TurnBoundary, TurnBuffer};
use jiff::Timestamp;
use std::path::PathBuf;
use uuid::Uuid;

fn evt(session: &str, turn: Option<&str>, kind: EventKind) -> AgentEvent {
    AgentEvent::V1(AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::ClaudeCode,
        session_id: session.into(),
        turn_id: turn.map(str::to_string),
        cwd: PathBuf::from("/tmp"),
        repo: None,
        occurred_at: Timestamp::now(),
        kind,
    })
}

#[test]
fn user_prompt_does_not_fire_boundary() {
    let mut buf = TurnBuffer::new();
    let b = buf.accept(&evt("s1", Some("t1"), EventKind::UserPrompt {
        text: "hi".into(), attachments: vec![],
    }));
    assert!(matches!(b, TurnBoundary::None));
}

#[test]
fn assistant_msg_with_usage_fires_boundary() {
    let mut buf = TurnBuffer::new();
    buf.accept(&evt("s1", Some("t1"), EventKind::UserPrompt { text: "hi".into(), attachments: vec![] }));
    let b = buf.accept(&evt("s1", Some("t1"), EventKind::AssistantMsg {
        text: "done".into(),
        truncated: false,
        token_usage: Some(TokenUsage { prompt_tokens: 10, completion_tokens: 5, cached_tokens: None }),
    }));
    match b {
        TurnBoundary::Fire { session_id, turn_id } => {
            assert_eq!(session_id, "s1");
            assert_eq!(turn_id.as_deref(), Some("t1"));
        }
        _ => panic!("expected Fire"),
    }
}

#[test]
fn assistant_msg_without_usage_does_not_fire() {
    let mut buf = TurnBuffer::new();
    let b = buf.accept(&evt("s1", Some("t1"), EventKind::AssistantMsg {
        text: "partial".into(), truncated: true, token_usage: None,
    }));
    assert!(matches!(b, TurnBoundary::None));
}

#[test]
fn session_end_fires_boundary() {
    let mut buf = TurnBuffer::new();
    buf.accept(&evt("s1", Some("t1"), EventKind::UserPrompt { text: "hi".into(), attachments: vec![] }));
    let b = buf.accept(&evt("s1", None, EventKind::SessionEnd { reason: "quit".into() }));
    assert!(matches!(b, TurnBoundary::Fire { .. }));
}

#[test]
fn new_user_prompt_fires_previous_turn() {
    let mut buf = TurnBuffer::new();
    buf.accept(&evt("s1", Some("t1"), EventKind::UserPrompt { text: "a".into(), attachments: vec![] }));
    buf.accept(&evt("s1", Some("t1"), EventKind::ToolCall {
        tool: "bash".into(), args_preview: "ls".into(),
        ok: true, duration_ms: 1, result_preview: "".into(),
    }));
    let b = buf.accept(&evt("s1", Some("t2"), EventKind::UserPrompt { text: "b".into(), attachments: vec![] }));
    // Previous t1 should fire because t2 is a different turn.
    match b {
        TurnBoundary::Fire { turn_id, .. } => assert_eq!(turn_id.as_deref(), Some("t1")),
        _ => panic!("expected Fire for prior turn"),
    }
}

#[test]
fn idle_timeout_fires_stale_turns() {
    let mut buf = TurnBuffer::new();
    buf.accept(&evt("s1", Some("t1"), EventKind::UserPrompt { text: "a".into(), attachments: vec![] }));
    let stale = buf.fire_idle_turns(std::time::Duration::from_secs(0));
    assert_eq!(stale.len(), 1);
    assert_eq!(stale[0].turn_id.as_deref(), Some("t1"));
}
