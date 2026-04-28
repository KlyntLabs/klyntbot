//! Phase 7 exit-gate: full CLI matrix parse roundtrip.

use coding_ingest::adapters::{claude_code::ClaudeCodeAdapter, codex::CodexAdapter, kimi_cli::KimiAdapter, IngestAdapter};
use coding_ingest::event::AgentEvent;

fn assert_roundtrip(adapter: &dyn IngestAdapter, event_name: &str, raw: &[u8]) {
    let parsed = adapter.parse(event_name, raw).expect("parse should not error");
    if let Some(event) = parsed {
        let json = serde_json::to_string(&event).expect("serialize");
        let deserialized: AgentEvent = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(event, deserialized, "roundtrip failed for {}", adapter.source_name());
    }
}

#[test]
fn claude_code_roundtrip() {
    let adapter = ClaudeCodeAdapter;
    let session_start = r#"{"session_id":"s1","cwd":"/tmp","model":"claude-sonnet-4","source":"cli"}"#;
    assert_roundtrip(&adapter, "SessionStart", session_start.as_bytes());
}

#[test]
fn codex_roundtrip() {
    let adapter = CodexAdapter;
    let session_start = r#"{"session_id":"s1","cwd":"/tmp","model":"gpt-4.1","source":"cli"}"#;
    assert_roundtrip(&adapter, "SessionStart", session_start.as_bytes());
}

#[test]
fn kimi_roundtrip() {
    let adapter = KimiAdapter;
    let session_start = r#"{"session_id":"s1","cwd":"/tmp","model":"kimi-k1.5","source":"cli"}"#;
    assert_roundtrip(&adapter, "SessionStart", session_start.as_bytes());
}
