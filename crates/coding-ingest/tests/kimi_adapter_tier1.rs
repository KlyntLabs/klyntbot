//! Tier-1 dispatch smoke tests — exercises every one of the 13 hook events.

use coding_ingest::adapters::kimi_cli::dispatch::dispatch;

fn ok(event: &str, payload: &str) {
    let parsed = dispatch(event, payload.as_bytes()).unwrap_or_else(|e| panic!("{event}: {e}"));
    assert!(parsed.is_some(), "{event} produced None");
}

#[test]
fn dispatch_covers_thirteen_events() {
    let common = r#""session_id":"s1","cwd":"/tmp""#;

    ok(
        "SessionStart",
        &format!("{{{common},\"model\":\"kimi\",\"source\":\"cli\"}}"),
    );
    ok("SessionEnd", &format!("{{{common},\"reason\":\"quit\"}}"));
    ok("UserPrompt", &format!("{{{common},\"prompt\":\"hi\"}}"));
    ok("AssistantMsg", &format!("{{{common},\"text\":\"hi\"}}"));
    ok(
        "ToolCall",
        &format!("{{{common},\"tool\":\"shell\",\"ok\":true,\"duration_ms\":3}}"),
    );
    ok(
        "FileEdit",
        &format!("{{{common},\"path\":\"/x.rs\",\"op\":\"modify\",\"bytes\":10}}"),
    );
    ok(
        "TestRun",
        &format!("{{{common},\"command\":\"cargo test\",\"passed\":1,\"failed\":0}}"),
    );
    ok(
        "CompactEvent",
        &format!("{{{common},\"trigger\":\"auto\"}}"),
    );
    ok("Error", &format!("{{{common},\"message\":\"boom\"}}"));
    ok(
        "SkillActivated",
        &format!("{{{common},\"skill_id\":\"task\",\"source_path\":\"/s/SKILL.md\"}}"),
    );
    ok("RecallInjected", &format!("{{{common}}}"));
    ok(
        "ApprovalDecision",
        &format!("{{{common},\"tool\":\"shell\",\"decision\":\"allow\"}}"),
    );
    ok(
        "ProviderCall",
        &format!("{{{common},\"model\":\"kimi\",\"prompt_tokens\":1,\"completion_tokens\":1}}"),
    );
}

#[test]
fn unknown_event_returns_none() {
    let r = dispatch(
        "DefinitelyUnknown",
        b"{\"session_id\":\"s\",\"cwd\":\"/x\"}",
    )
    .unwrap();
    assert!(r.is_none());
}
