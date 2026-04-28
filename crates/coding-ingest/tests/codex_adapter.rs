//! Per-adapter smoke tests for the Codex hook adapter.

use coding_ingest::adapters::{codex::CodexAdapter, IngestAdapter};
use coding_ingest::event::{AgentEvent, AgentSource, EventKind};

fn parse_or_panic(adapter: &CodexAdapter, name: &str, raw: &[u8]) -> AgentEvent {
    adapter
        .parse(name, raw)
        .expect("parse should not error")
        .expect("event should be produced")
}

fn assert_codex_source(event: &AgentEvent) {
    let AgentEvent::V1(v1) = event;
    assert_eq!(v1.source, AgentSource::Codex);
}

#[test]
fn source_name_is_codex() {
    assert_eq!(CodexAdapter.source_name(), "codex");
}

#[test]
fn unknown_event_returns_none() {
    let adapter = CodexAdapter;
    let parsed = adapter
        .parse("DefinitelyNotARealEvent", b"{}")
        .expect("parse");
    assert!(parsed.is_none());
}

#[test]
fn session_start_parses() {
    let adapter = CodexAdapter;
    let raw = br#"{"session_id":"s1","cwd":"/tmp","model":"gpt-4.1","source":"cli"}"#;
    let event = parse_or_panic(&adapter, "SessionStart", raw);
    assert_codex_source(&event);
    let AgentEvent::V1(v1) = &event;
    assert!(matches!(v1.kind, EventKind::SessionStart { .. }));
}
