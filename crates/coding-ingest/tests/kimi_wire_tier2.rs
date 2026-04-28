//! Tier-2 Wire client smoke test — verifies frame-to-event translation.

use coding_ingest::adapters::kimi_cli::wire::{frame_to_event, WireFrame};
use coding_ingest::event::AgentEvent;

#[test]
fn frame_to_event_user_prompt_roundtrip() {
    let frame = WireFrame {
        frame_type: "user_prompt".into(),
        session_id: "s1".into(),
        payload: serde_json::json!({
            "session_id": "s1",
            "cwd": "/tmp",
            "prompt": "hello",
            "attachments": []
        }),
    };
    let evt = frame_to_event(&frame).unwrap().expect("frame produced an event");
    let AgentEvent::V1(v1) = evt;
    assert_eq!(v1.session_id, "s1");
}

#[test]
fn frame_to_event_unknown_type_returns_none() {
    let frame = WireFrame {
        frame_type: "exotic_unknown".into(),
        session_id: "s1".into(),
        payload: serde_json::Value::Null,
    };
    assert!(frame_to_event(&frame).unwrap().is_none());
}
