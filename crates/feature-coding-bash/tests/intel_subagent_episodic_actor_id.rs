//! Phase 2.3b: episode for a subagent-spawned job uses the subagent's agent_id as actor_id.

use cognitive::mirror::sources::coding_bash::build_episodic_memory;
use jiff::Timestamp;
use storage::repos::BashJobRow;

fn fake_row(agent_id: &str) -> BashJobRow {
    BashJobRow {
        id: "bash-x".into(),
        session_id: "s1".into(),
        agent_id: agent_id.into(),
        description: "d".into(),
        command: "c".into(),
        command_key: "k".into(),
        cwd: "/".into(),
        timeout_ms: 60_000,
        silent_completion: false,
        status: "Completed".into(),
        exit_code: Some(0),
        failure_kind: None,
        failure_detail: None,
        failure_extracted: None,
        started_at: Timestamp::from_millisecond(1_700_000_000_000).unwrap(),
        finished_at: Some(Timestamp::from_millisecond(1_700_000_001_000).unwrap()),
        total_bytes_emitted: 0,
        bisect_count: 0,
        log_path: "/tmp/x.log".into(),
        final_path: None,
        last_polled_at: None,
        last_seen_offset: 0,
    }
}

#[test]
fn actor_id_matches_subagent() {
    let mem = build_episodic_memory(&fake_row("subagent-7"));
    assert_eq!(mem.actor_id, Some("subagent-7".into()));
}
