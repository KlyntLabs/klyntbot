use coding_ingest::adapters::claude_code::ClaudeCodeAdapter;
use coding_ingest::adapters::IngestAdapter;
use coding_ingest::event::{AgentEvent, EventKind, FileOp};

#[test]
fn edit_emits_diff_preview() {
    let adapter = ClaudeCodeAdapter::default();
    let raw = serde_json::json!({
        "session_id": "s1",
        "cwd": "/tmp",
        "tool_name": "Edit",
        "tool_input": {
            "file_path": "/tmp/x.rs",
            "old_string": "fn old() {}",
            "new_string": "fn new() {}"
        },
        "tool_response": {"bytes": 42},
        "duration_ms": 5
    });
    let bytes = serde_json::to_vec(&raw).unwrap();
    let evt = adapter.parse("PostToolUse", &bytes).unwrap().unwrap();
    let AgentEvent::V1(v1) = evt;
    if let EventKind::FileEdit {
        op, diff_preview, ..
    } = v1.kind
    {
        assert_eq!(op, FileOp::Modify);
        let preview = diff_preview.expect("diff_preview should be Some for Edit");
        assert!(preview.contains("-fn old"));
        assert!(preview.contains("+fn new"));
    } else {
        panic!("expected FileEdit");
    }
}
