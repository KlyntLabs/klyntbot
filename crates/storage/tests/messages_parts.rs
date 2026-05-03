use serde_json::json;
use storage::messages::parts::{MessagePart, ToolOutput};

#[test]
fn message_part_text_round_trip() {
    let p = MessagePart::Text {
        text: "hello".into(),
    };
    let s = serde_json::to_string(&p).unwrap();
    let back: MessagePart = serde_json::from_str(&s).unwrap();
    match back {
        MessagePart::Text { text } => assert_eq!(text, "hello"),
        other => panic!("expected Text, got {other:?}"),
    }
}

#[test]
fn message_part_serializes_kind_tag() {
    let p = MessagePart::ToolCall {
        call_id: "c1".into(),
        name: "bash".into(),
        args: json!({"cmd": "ls"}),
    };
    let s = serde_json::to_string(&p).unwrap();
    assert!(s.contains("\"kind\":\"tool_call\""), "got: {s}");
    assert!(s.contains("\"call_id\":\"c1\""));
}

#[test]
fn message_part_command_execution_carries_streams() {
    let p = MessagePart::CommandExecution(Box::new(storage::messages::parts::CommandExecutionData {
        command: vec!["cargo".into(), "test".into()],
        cwd: "/tmp".into(),
        exit_code: Some(0),
        stdout: "ok".into(),
        stderr: String::new(),
    }));
    let s = serde_json::to_string(&p).unwrap();
    let back: MessagePart = serde_json::from_str(&s).unwrap();
    match back {
        MessagePart::CommandExecution(data) if data.exit_code == Some(0) => (),
        other => panic!("expected CommandExecution exit 0, got {other:?}"),
    }
}
