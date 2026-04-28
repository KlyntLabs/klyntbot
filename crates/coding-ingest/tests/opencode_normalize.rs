use coding_ingest::adapters::opencode::{normalize, schema::MessageRow};
use coding_ingest::event::{AgentSource, EventKind};

#[test]
fn normalize_recovers_cwd_from_metadata() {
    let row = MessageRow {
        id: 1,
        session_id: "s1".into(),
        role: "user".into(),
        content: "what does this repo do".into(),
        tool_calls: None,
        tool_call_id: None,
        metadata: Some(serde_json::json!({"cwd": "/Users/jayden/Projects/Klynt/bot"}).to_string()),
        created_at: "1700000000".into(),
    };
    let v1 = normalize::row_to_event(row).unwrap().unwrap();
    assert_eq!(
        v1.cwd,
        std::path::PathBuf::from("/Users/jayden/Projects/Klynt/bot")
    );
    assert!(
        v1.repo.is_some(),
        "repo should resolve from cwd via RepoScope"
    );
    assert_eq!(v1.source, AgentSource::OpenCode);
}

#[test]
fn normalize_groups_into_turns_by_session_and_assistant_boundary() {
    let user = MessageRow {
        id: 10,
        session_id: "s1".into(),
        role: "user".into(),
        content: "hello".into(),
        tool_calls: None,
        tool_call_id: None,
        metadata: None,
        created_at: "1700000000".into(),
    };
    let assistant = MessageRow {
        id: 11,
        session_id: "s1".into(),
        role: "assistant".into(),
        content: "hi back".into(),
        tool_calls: None,
        tool_call_id: None,
        metadata: None,
        created_at: "1700000005".into(),
    };
    let u_evt = normalize::row_to_event(user).unwrap().unwrap();
    let a_evt = normalize::row_to_event(assistant).unwrap().unwrap();
    assert_eq!(u_evt.turn_id, a_evt.turn_id);
    assert!(u_evt.turn_id.is_some());
}

#[test]
fn assistant_with_tool_calls_column_classifies_as_toolcall_not_heuristic() {
    let row = MessageRow {
        id: 1,
        session_id: "s1".into(),
        role: "assistant".into(),
        content: "{ \"this is just JSON the model returned, not a tool call\": true }".into(),
        tool_calls: None, // ← no actual tool call
        tool_call_id: None,
        metadata: None,
        created_at: "1700000000".into(),
    };
    let v1 = normalize::row_to_event(row).unwrap().unwrap();
    assert!(
        matches!(v1.kind, EventKind::AssistantMsg { .. }),
        "must NOT classify as ToolCall when tool_calls column is empty"
    );
}
