//! Phase 7 exit-gate: full CLI matrix parse roundtrip.
//!
//! Hook-driven CLIs (claude-code, codex, kimi-cli) are exercised through their
//! adapter `parse()` paths. OpenCode is poll-only (no hook adapter), and
//! KlyntCli is the native source emitted internally — for these we roundtrip
//! synthetic events tagged with the appropriate `AgentSource` to prove the
//! serialization invariant holds for every cross-CLI source.

use coding_ingest::adapters::{
    claude_code::ClaudeCodeAdapter, codex::CodexAdapter, kimi_cli::KimiAdapter, IngestAdapter,
};
use coding_ingest::event::{AgentEvent, AgentEventV1, AgentSource, EventKind};
use jiff::Timestamp;
use std::path::PathBuf;
use uuid::Uuid;

fn assert_adapter_roundtrip(adapter: &dyn IngestAdapter, event_name: &str, raw: &[u8]) {
    let parsed = adapter.parse(event_name, raw).expect("parse should not error");
    if let Some(event) = parsed {
        let json = serde_json::to_string(&event).expect("serialize");
        let deserialized: AgentEvent = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            event,
            deserialized,
            "roundtrip failed for {}",
            adapter.source_name()
        );
    }
}

fn assert_source_roundtrip(source: AgentSource, kind: EventKind) {
    let event = AgentEvent::V1(AgentEventV1 {
        id: Uuid::nil(),
        source,
        session_id: "matrix-s".into(),
        turn_id: Some("matrix-t".into()),
        cwd: PathBuf::from("/tmp"),
        repo: None,
        occurred_at: Timestamp::from_second(1_700_000_000).unwrap(),
        kind,
    });
    let json = serde_json::to_string(&event).expect("serialize");
    let back: AgentEvent = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(event, back, "source roundtrip failed for {:?}", source);
}

#[test]
fn claude_code_roundtrip() {
    let adapter = ClaudeCodeAdapter;
    let session_start = r#"{"session_id":"s1","cwd":"/tmp","model":"claude-sonnet-4","source":"cli"}"#;
    assert_adapter_roundtrip(&adapter, "SessionStart", session_start.as_bytes());
}

#[test]
fn codex_roundtrip() {
    let adapter = CodexAdapter;
    let session_start = r#"{"session_id":"s1","cwd":"/tmp","model":"gpt-4.1","source":"cli"}"#;
    assert_adapter_roundtrip(&adapter, "SessionStart", session_start.as_bytes());
}

#[test]
fn kimi_roundtrip() {
    let adapter = KimiAdapter;
    let session_start = r#"{"session_id":"s1","cwd":"/tmp","model":"kimi-k1.5","source":"cli"}"#;
    assert_adapter_roundtrip(&adapter, "SessionStart", session_start.as_bytes());
}

#[test]
fn opencode_source_roundtrip() {
    assert_source_roundtrip(
        AgentSource::OpenCode,
        EventKind::UserPrompt {
            text: "opencode prompt".into(),
            attachments: vec![],
        },
    );
}

#[test]
fn klynt_cli_source_roundtrip() {
    assert_source_roundtrip(
        AgentSource::KlyntCli,
        EventKind::SessionEnd {
            reason: "klynt-native".into(),
        },
    );
}
