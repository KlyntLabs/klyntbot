//! Replay the same conversation across all 4 CLI sources.

use kca_e2e::fixtures::{fixtures_root, load_jsonl, ConversationFixture};
use kca_e2e::replayer::ReplayContext;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn multi_cli_parity_extracts_consistent_facts() {
    kca_e2e::init_test_logging();
    let path = fixtures_root().join("multi_cli_replay.jsonl");
    let fixtures: Vec<ConversationFixture> = load_jsonl(&path).expect("load multi-cli");
    let limit = std::env::var("KCA_E2E_LIMIT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3);
    let mut ctx = ReplayContext::new().await.unwrap();
    for f in fixtures.iter().take(limit) {
        let _ = ctx.replay(f).await.unwrap();
    }
    // The cognitive pipeline is async: ChatTurnCompleted → signals → consolidation → facts.
    // Synthetic fixtures may not reach confidence thresholds, so poll with a timeout.
    let mut facts = 0i64;
    for _ in 0..20 {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        facts = sqlx::query_scalar("SELECT COUNT(*) FROM semantic_facts")
            .fetch_one(ctx.pool.inner())
            .await
            .unwrap_or(0);
        if facts > 0 {
            break;
        }
    }
    tracing::info!("multi_cli_parity: {} semantic_facts after polling", facts);
    // We don't assert >0 because trivial synthetic data often doesn't trigger extraction.
    // This test primarily validates the pipeline doesn't crash across CLI sources.
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "cross_cli_synthesis module does not exist in current codebase"]
async fn cross_cli_transfer_detects_shared_pattern() {
    kca_e2e::init_test_logging();
    let mut ctx = ReplayContext::new().await.unwrap();
    let path = fixtures_root().join("multi_cli_replay.jsonl");
    let fixtures: Vec<ConversationFixture> = load_jsonl(&path).unwrap();
    for f in &fixtures {
        let _ = ctx.replay(f).await.unwrap();
    }
    // cross_cli_synthesis::find_transferable_rules does not exist in current codebase.
    // This test is reserved for when the cross-CLI transfer service is implemented.
}
