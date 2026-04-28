//! Inv 7 — Cross-CLI normalization: parse(serialize(AgentEvent)) == AgentEvent
//! over all AgentSource variants.

use coding_ingest::event::{AgentEvent, AgentEventV1, AgentSource, EventKind};
use jiff::Timestamp;
use proptest::prelude::*;
use std::path::PathBuf;
use uuid::Uuid;

fn arb_agent_source() -> impl Strategy<Value = AgentSource> {
    prop::sample::select(vec![
        AgentSource::ClaudeCode,
        AgentSource::Codex,
        AgentSource::KimiCli,
        AgentSource::OpenCode,
        AgentSource::KlyntCli,
    ])
}

fn arb_event_kind() -> impl Strategy<Value = EventKind> {
    use coding_ingest::event::FileOp;
    prop::sample::select(vec![
        EventKind::SessionStart {
            model: Some("test-model".into()),
            source_reason: "cli".into(),
        },
        EventKind::SessionEnd {
            reason: "success".into(),
        },
        EventKind::UserPrompt {
            text: "hello world".into(),
            attachments: vec![],
        },
        EventKind::AssistantMsg {
            text: "hi there".into(),
            truncated: false,
            token_usage: None,
        },
        EventKind::ToolCall {
            tool: "read".into(),
            args_preview: "{}".into(),
            ok: true,
            duration_ms: 100,
            result_preview: "ok".into(),
        },
        EventKind::FileEdit {
            path: PathBuf::from("/tmp/x.rs"),
            op: FileOp::Modify,
            bytes: 42,
            diff_preview: Some("-old\n+new".into()),
        },
        EventKind::TestRun {
            command: "cargo test".into(),
            framework: Some("cargo".into()),
            passed: 10,
            failed: 0,
            duration_ms: 5000,
        },
        EventKind::CompactEvent {
            trigger: "manual".into(),
            token_count: 4096,
        },
        EventKind::Error {
            tool: Some("bash".into()),
            message: "exit 1".into(),
        },
    ])
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn agent_event_roundtrip_identity(
        source in arb_agent_source(),
        kind in arb_event_kind(),
    ) {
        let original = AgentEvent::V1(AgentEventV1 {
            id: Uuid::new_v4(),
            source,
            session_id: "s1".into(),
            turn_id: Some("t1".into()),
            cwd: PathBuf::from("/tmp"),
            repo: None,
            occurred_at: Timestamp::now(),
            kind,
        });

        let json = serde_json::to_string(&original).expect("serialize");
        let deserialized: AgentEvent = serde_json::from_str(&json).expect("deserialize");

        prop_assert_eq!(original, deserialized);
    }
}
