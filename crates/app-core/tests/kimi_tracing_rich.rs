use std::path::PathBuf;
use std::sync::Arc;

use app_core::tracing::{providers::kimi::KimiTracingProvider, TracingProvider};
use coding_ingest::adapters::kimi_cli::workdir::WorkdirIndex;

fn fixture_paths() -> (PathBuf, PathBuf, PathBuf) {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/fixtures/kimi");
    let kimi_root = p.join("sessions");
    let kimi_json = p.join("kimi.json");
    let imported = tempfile::tempdir().unwrap().keep();
    (kimi_root, kimi_json, imported)
}

#[tokio::test]
async fn rich_fixture_loads_and_aggregates() {
    let (kimi_root, kimi_json, imported) = fixture_paths();
    let widx = Arc::new(WorkdirIndex::new());
    widx.refresh(&kimi_json).await.unwrap();
    let provider = KimiTracingProvider::new_for_test(kimi_root, kimi_json, imported, widx);

    let summary = provider.session_summary("sess-fixture-rich").await.unwrap();
    assert!(summary.turn_count >= 3);
    assert!(summary.tool_call_count >= 8);
    assert!(summary.error_count >= 1);
    assert_eq!(summary.subagent_count, 2);

    let detail = provider
        .load_session("sess-fixture-rich", app_core::tracing::Scope::Main)
        .await
        .unwrap();
    assert!(detail.events.len() >= 100);

    let subs = provider.list_subagents("sess-fixture-rich").await.unwrap();
    assert_eq!(subs.len(), 2);
}
