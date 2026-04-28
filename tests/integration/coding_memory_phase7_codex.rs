//! Phase 7 integration test: Codex adapter ingests synthetic session.

use coding_ingest::adapters::codex::CodexAdapter;
use coding_ingest::adapters::IngestAdapter;

#[test]
fn codex_session_parses_all_events() {
    let adapter = CodexAdapter;
    let fixture = include_str!("../fixtures/coding/synthetic_session_codex.jsonl");
    let mut count = 0;
    for line in fixture.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        let event_name = v["event"].as_str().unwrap();
        let raw = line.as_bytes();
        let result = adapter.parse(event_name, raw);
        assert!(result.is_ok(), "parse failed for {event_name}: {result:?}");
        if let Ok(Some(_)) = result {
            count += 1;
        }
    }
    assert!(count >= 4, "expected >=4 parsed events, got {count}");
}
