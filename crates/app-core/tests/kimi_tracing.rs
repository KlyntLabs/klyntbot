use std::path::PathBuf;
use std::sync::Arc;

use app_core::tracing::{providers::kimi::KimiTracingProvider, Scope, TracingProvider};
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
async fn kimi_provider_end_to_end() {
    let (kimi_root, kimi_json, imported) = fixture_paths();
    let widx = Arc::new(WorkdirIndex::new());
    widx.refresh(&kimi_json).await.unwrap();
    let provider = KimiTracingProvider::new_for_test(kimi_root, kimi_json, imported, widx);

    let sessions = provider.list_sessions().await.unwrap();
    assert_eq!(sessions.len(), 2);
    let s = sessions
        .iter()
        .find(|s| s.session_id == "sess-fixture-001")
        .expect("sess-fixture-001 not found");
    assert_eq!(s.turn_count, 1);

    let detail = provider
        .load_session(&s.session_id, Scope::Main)
        .await
        .unwrap();
    assert_eq!(detail.events.len(), 9);

    let ctx = provider
        .load_context(&s.session_id, Scope::Main)
        .await
        .unwrap();
    assert_eq!(ctx.len(), 5);

    let state = provider.load_state(&s.session_id).await.unwrap();
    assert_eq!(state.todos.len(), 2);

    let subs = provider.list_subagents(&s.session_id).await.unwrap();
    assert_eq!(subs.len(), 1);
    assert_eq!(subs[0].agent_id, "sub-aaa");

    let sub_detail = provider
        .load_session(
            &s.session_id,
            Scope::Subagent {
                agent_id: "sub-aaa".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(sub_detail.events.len(), 4);
}
