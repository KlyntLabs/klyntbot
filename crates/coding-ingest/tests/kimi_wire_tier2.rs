//! Tier-2 Wire client smoke test — verifies the line-decoder roundtrip.

use coding_ingest::adapters::kimi_cli::wire::WireClient;
use coding_ingest::event::{AgentEventV1, AgentSource, EventKind};
use jiff::Timestamp;
use std::path::PathBuf;
use uuid::Uuid;

#[test]
fn decode_frame_roundtrip() {
    let event = AgentEventV1 {
        id: Uuid::nil(),
        source: AgentSource::KimiCli,
        session_id: "s".into(),
        turn_id: None,
        cwd: PathBuf::from("/tmp"),
        repo: None,
        occurred_at: Timestamp::from_second(0).unwrap(),
        kind: EventKind::SessionEnd {
            reason: "wire".into(),
        },
    };
    let line = serde_json::to_string(&event).unwrap();
    let back = WireClient::decode_frame(&line).expect("decode");
    assert_eq!(back, event);
}

#[test]
fn decode_frame_strips_trailing_newline() {
    let event = AgentEventV1 {
        id: Uuid::nil(),
        source: AgentSource::KimiCli,
        session_id: "s".into(),
        turn_id: None,
        cwd: PathBuf::from("/tmp"),
        repo: None,
        occurred_at: Timestamp::from_second(0).unwrap(),
        kind: EventKind::SessionEnd {
            reason: "x".into(),
        },
    };
    let mut line = serde_json::to_string(&event).unwrap();
    line.push('\n');
    let back = WireClient::decode_frame(&line).expect("decode");
    assert_eq!(back.session_id, "s");
}
