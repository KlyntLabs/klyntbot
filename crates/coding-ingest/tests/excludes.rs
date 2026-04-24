use coding_ingest::event::{AgentEvent, AgentEventV1, AgentSource, EventKind, FileOp};
use coding_ingest::excludes::{default_exclude_globs, ExcludeSet};
use jiff::Timestamp;
use std::path::PathBuf;
use uuid::Uuid;

fn file_edit(path: &str) -> AgentEvent {
    AgentEvent::V1(AgentEventV1 {
        id: Uuid::new_v4(), source: AgentSource::ClaudeCode,
        session_id: "s".into(), turn_id: None,
        cwd: PathBuf::from("/tmp"), repo: None,
        occurred_at: Timestamp::now(),
        kind: EventKind::FileEdit {
            path: PathBuf::from(path), op: FileOp::Modify,
            bytes: 0, diff_preview: None,
        },
    })
}

#[test]
fn defaults_block_env_and_keys() {
    let s = ExcludeSet::compile(&default_exclude_globs()).unwrap();
    assert!(s.should_drop(&file_edit("/home/u/proj/.env")));
    assert!(s.should_drop(&file_edit("/home/u/proj/secrets/db.toml")));
    assert!(s.should_drop(&file_edit("/home/u/.ssh/id_rsa")));
    assert!(s.should_drop(&file_edit("/home/u/proj/node_modules/x/y.js")));
    assert!(!s.should_drop(&file_edit("/home/u/proj/src/main.rs")));
}

#[test]
fn tool_call_args_are_scanned() {
    let s = ExcludeSet::compile(&["**/*.key".to_string()]).unwrap();
    let evt = AgentEvent::V1(AgentEventV1 {
        id: Uuid::new_v4(), source: AgentSource::ClaudeCode,
        session_id: "s".into(), turn_id: None,
        cwd: PathBuf::from("/tmp"), repo: None,
        occurred_at: Timestamp::now(),
        kind: EventKind::ToolCall {
            tool: "Read".into(),
            args_preview: "path=/home/u/keys/deploy.key".into(),
            ok: true, duration_ms: 2, result_preview: String::new(),
        },
    });
    assert!(s.should_drop(&evt));
}
