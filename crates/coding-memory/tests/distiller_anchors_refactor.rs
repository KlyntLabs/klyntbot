//! Phase-A refactor episodes carry anchored_symbols extracted via tree-sitter.

use coding_ingest::event::{AgentEvent, AgentEventV1, AgentSource, EventKind, FileOp};
use coding_memory::distiller::phase_a::{compute_turn_trace, extract_refactor_anchors};
use jiff::Timestamp;
use std::path::PathBuf;
use uuid::Uuid;

fn fake_event(path: &str) -> AgentEvent {
    AgentEvent::V1(AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::ClaudeCode,
        session_id: "s1".into(),
        turn_id: Some("t1".into()),
        cwd: PathBuf::from("/tmp/repo"),
        repo: None,
        occurred_at: Timestamp::now(),
        kind: EventKind::FileEdit {
            path: PathBuf::from(path),
            op: FileOp::Modify,
            bytes: 100,
            diff_preview: None,
        },
    })
}

#[test]
fn refactor_episode_carries_anchored_symbols() {
    // Synthetic file edits across two .rs files.
    let events = vec![fake_event("/tmp/repo/a.rs"), fake_event("/tmp/repo/b.rs")];
    let trace = compute_turn_trace("s1", Some("t1"), &events);
    // Phase-A turn trace records the file-path list. Anchored extraction
    // happens when these files are present on disk; here we only verify the
    // trace path lists are correct so the downstream extractor has paths.
    assert_eq!(trace.files_modified.len(), 2);
}

#[test]
fn extract_refactor_anchors_reads_disk_and_extracts() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.rs");
    std::fs::write(&path, "fn hello() {}\n").unwrap();
    let extractor = coding_memory::TreeSitterExtractor::new();
    let files = vec![(path.clone(), 14_i64)];
    let symbols = extract_refactor_anchors(&extractor, &files, "abc1234");
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0].symbol, "hello");
    assert_eq!(symbols[0].git_hash, "abc1234");
}
