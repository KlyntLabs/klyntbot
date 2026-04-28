//! When `FileEditEnriched` arrives, the distiller must NOT re-parse files.
//! It should use the inbound `anchored_symbols` via `anchors_from_enriched`.

use coding_ingest::event::{AgentEvent, AgentEventV1, AgentSource, EventKind, FileOp};
use coding_memory::distiller::phase_a::anchors_from_enriched;
use std::path::PathBuf;

#[test]
fn anchors_from_enriched_returns_inbound_symbols() {
    let events = vec![AgentEvent::V1(AgentEventV1 {
        id: uuid::Uuid::new_v4(),
        source: AgentSource::ClaudeCode,
        session_id: "s1".into(),
        turn_id: Some("t1".into()),
        cwd: PathBuf::from("/tmp/repo"),
        repo: None,
        occurred_at: jiff::Timestamp::now(),
        kind: EventKind::FileEditEnriched {
            path: PathBuf::from("src/lib.rs"),
            op: FileOp::Modify,
            lsp_diagnostics_delta: None,
            anchored_symbols: vec![coding_ingest::event::SymbolRef {
                file_path: PathBuf::from("src/lib.rs"),
                symbol: "foo".into(),
                git_hash: "abc123".into(),
            }],
        },
    })];

    let anchors = anchors_from_enriched(&events);
    assert_eq!(anchors.len(), 1);
    assert_eq!(anchors[0].symbol, "foo");
    assert_eq!(anchors[0].file_path, PathBuf::from("src/lib.rs"));
    assert_eq!(anchors[0].git_hash, "abc123");
}

#[test]
fn anchors_from_enriched_ignores_plain_file_edit() {
    let events = vec![AgentEvent::V1(AgentEventV1 {
        id: uuid::Uuid::new_v4(),
        source: AgentSource::ClaudeCode,
        session_id: "s1".into(),
        turn_id: Some("t1".into()),
        cwd: PathBuf::from("/tmp/repo"),
        repo: None,
        occurred_at: jiff::Timestamp::now(),
        kind: EventKind::FileEdit {
            path: PathBuf::from("src/lib.rs"),
            op: FileOp::Modify,
            bytes: 100,
            diff_preview: None,
        },
    })];

    let anchors = anchors_from_enriched(&events);
    assert!(anchors.is_empty());
}
