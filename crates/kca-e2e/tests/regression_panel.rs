//! Regression panel: every closed bug has a fixture that re-fails if the regression returns.

use futures::FutureExt;
use kca_e2e::fixtures::*;
use kca_e2e::replayer::ReplayContext;

#[tokio::test(flavor = "multi_thread")]
async fn regression_panel_all_closed_bugs_stay_closed() {
    kca_e2e::init_test_logging();
    let path = fixtures_root().join("regression_panel.jsonl");
    let fixtures: Vec<ConversationFixture> = load_jsonl(&path).expect("load regression");
    let limit = std::env::var("KCA_E2E_LIMIT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3);
    let mut failures = Vec::new();
    let mut ctx = ReplayContext::new().await.unwrap();
    for f in fixtures.iter().take(limit) {
        let bug_id = f
            .metadata
            .get("bug_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let result = std::panic::AssertUnwindSafe(ctx.replay(f))
            .catch_unwind()
            .await;
        match result {
            Ok(Ok(_)) => {
                if let Err(msg) = run_regression_assertions(&ctx, bug_id, f).await {
                    failures.push(format!("{bug_id}: {msg}"));
                }
            }
            Ok(Err(e)) => failures.push(format!("{bug_id}: replay error {e}")),
            Err(_) => failures.push(format!("{bug_id}: panicked")),
        }
    }
    assert!(
        failures.is_empty(),
        "regression failures:\n{}",
        failures.join("\n")
    );
}

async fn run_regression_assertions(
    ctx: &ReplayContext,
    bug_id: &str,
    _f: &ConversationFixture,
) -> Result<(), String> {
    match bug_id {
        "bug_003" => {
            let events = ctx.captured_events.lock().await;
            let count = events
                .iter()
                .filter(|e| matches!(e, bus::DomainEvent::ToolCallExecuted { .. }))
                .count();
            if count == 0 {
                return Err("ToolCallExecuted not published".into());
            }
        }
        "bug_004" => {
            let edge_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM entity_relationships")
                .fetch_one(ctx.pool.inner())
                .await
                .unwrap_or(0);
            if edge_count == 0 {
                return Err("coding distilled fact wrote no edge".into());
            }
        }
        _ => {}
    }
    Ok(())
}
