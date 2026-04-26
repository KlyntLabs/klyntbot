use coding_memory::distiller::phase_b::build_prompt;
use coding_memory::distiller::{TestOutcome, TurnTokenUsage, TurnTrace};
use jiff::Timestamp;
use std::path::PathBuf;

fn trace() -> TurnTrace {
    TurnTrace {
        session_id: "s".into(),
        turn_id: Some("t".into()),
        files_read: vec![PathBuf::from("src/main.rs")],
        files_modified: vec![(PathBuf::from("src/parser.rs"), 42)],
        commands_run: vec!["cargo test".into()],
        test_outcomes: vec![TestOutcome {
            command: "cargo test".into(),
            framework: Some("cargo".into()),
            passed: 10,
            failed: 0,
        }],
        errors_encountered: vec![],
        token_usage: Some(TurnTokenUsage {
            prompt: 100,
            completion: 50,
            cached: 0,
        }),
        started_at: Timestamp::now(),
        ended_at: Some(Timestamp::now()),
    }
}

#[test]
fn prompt_contains_user_text_and_assistant_text() {
    let p = build_prompt(
        "fix the parser",
        "I edited parser.rs and added a null guard.",
        &trace(),
        Some("github.com/klynt/bot"),
    );
    assert!(p.system.contains("memory distiller"));
    assert!(p.user_message.contains("fix the parser"));
    assert!(p.user_message.contains("I edited parser.rs"));
    assert!(p.user_message.contains("src/parser.rs"));
    assert!(p.user_message.contains("cargo test"));
    assert!(p.user_message.contains("github.com/klynt/bot"));
}

#[test]
fn prompt_truncates_extreme_inputs() {
    let huge = "x".repeat(50_000);
    let p = build_prompt(&huge, &huge, &trace(), None);
    // Our safety cap is well under 50k chars.
    assert!(p.user_message.len() < 30_000);
}
