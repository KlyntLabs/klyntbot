//! Integration test for the Claude Code tracing provider against a hand-crafted fixture.

use app_core::tracing::provider::TracingProvider;
use app_core::tracing::providers::claude_code::ClaudeCodeTracingProvider;
use app_core::tracing::types::{HeaderChip, Scope, SessionTab};
use std::path::PathBuf;

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/claude_code/projects")
}

fn imported_root() -> PathBuf {
    tempfile::tempdir().unwrap().keep()
}

#[tokio::test]
async fn declares_two_tabs_and_claude_code_header_chips() {
    let p = ClaudeCodeTracingProvider::new(fixture_root(), imported_root());
    assert_eq!(p.id(), "claudeCode");
    assert_eq!(p.supported_tabs(), &[SessionTab::Wire, SessionTab::Agents]);
    assert!(p.header_layout().contains(&HeaderChip::Model));
    assert!(p.header_layout().contains(&HeaderChip::Compactions));
    assert!(!p.header_layout().contains(&HeaderChip::Steps));
}

#[tokio::test]
async fn list_sessions_finds_fixture() {
    let p = ClaudeCodeTracingProvider::new(fixture_root(), imported_root());
    let sessions = p.list_sessions().await.unwrap();
    let s = sessions
        .iter()
        .find(|s| s.session_id == "sess1")
        .expect("sess1");
    assert_eq!(s.provider_id, "claudeCode");
    assert_eq!(s.project_basename.as_deref(), Some("fixture"));
    assert_eq!(s.custom_title.as_deref(), Some("Refactor session"));
}

#[tokio::test]
async fn load_session_emits_expected_counts() {
    let p = ClaudeCodeTracingProvider::new(fixture_root(), imported_root());
    let detail = p.load_session("sess1", Scope::Main).await.unwrap();
    assert_eq!(detail.stats.turn_count, 2);
    assert_eq!(detail.stats.tool_call_count, 2);
    assert_eq!(detail.stats.error_count, 2, "tool_result error + api_error");
    assert_eq!(detail.stats.compaction_count, 1);
    assert_eq!(detail.stats.model.as_deref(), Some("claude-opus-4-7"));
    assert!(detail.stats.total_input_tokens > 0);
    assert!(detail.stats.cache_hit_pct > 0.0);
}

#[tokio::test]
async fn list_subagents_returns_meta() {
    let p = ClaudeCodeTracingProvider::new(fixture_root(), imported_root());
    let subs = p.list_subagents("sess1").await.unwrap();
    assert_eq!(subs.len(), 1);
    assert_eq!(subs[0].agent_id, "AGENT1");
    assert_eq!(subs[0].subagent_type, "superpowers:code-reviewer");
    assert_eq!(subs[0].description.as_deref(), Some("Verify the refactor"));
}

#[tokio::test]
async fn load_subagent_session_works() {
    let p = ClaudeCodeTracingProvider::new(fixture_root(), imported_root());
    let detail = p
        .load_session(
            "sess1",
            Scope::Subagent {
                agent_id: "AGENT1".into(),
            },
        )
        .await
        .unwrap();
    assert!(!detail.events.is_empty());
}

#[tokio::test]
async fn unsupported_methods_return_not_implemented() {
    let p = ClaudeCodeTracingProvider::new(fixture_root(), imported_root());
    assert!(p.load_state("sess1").await.is_err());
    assert!(p.load_context("sess1", Scope::Main).await.is_err());
    assert!(p.load_subagent_context("sess1", "AGENT1").await.is_err());
}

#[tokio::test]
async fn import_round_trip() {
    let p = ClaudeCodeTracingProvider::new(fixture_root(), imported_root());
    let src = fixture_root().join("-tmp-fixture/sess1.jsonl");
    let id = p.import_from_file(&src).await.unwrap();
    assert!(!id.is_empty());
}
