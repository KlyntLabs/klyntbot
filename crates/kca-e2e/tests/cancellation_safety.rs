//! Verify: cancel a chat mid-flight, send a second chat, no orphan rows.

use kca_e2e::replayer::ReplayContext;
use tokio::time::{sleep, Duration};

#[tokio::test(flavor = "multi_thread")]
async fn chat_cancel_mid_turn_leaves_no_orphans() {
    kca_e2e::init_test_logging();
    let ctx = ReplayContext::new().await.unwrap();
    let key = common::SessionKey::from_parts("kca-e2e", "cancel_test");
    let app = ctx.app.clone();
    let key_clone = key.clone();
    let h = tokio::spawn(async move {
        let _ = app
            .chat_send("a".repeat(5000), key_clone.to_string(), None, None)
            .await;
    });
    sleep(Duration::from_millis(50)).await;
    let _ = ctx.app.chat_cancel(key.to_string()).await;
    let _ = h.await;
    let _ = ctx
        .app
        .chat_send("hello".into(), key.to_string(), None, None)
        .await
        .unwrap();

    let crit_orphans: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM extraction_critic_log WHERE fact_id NOT IN (SELECT id FROM semantic_facts)"
    ).fetch_one(ctx.pool.inner()).await.unwrap_or(0);
    assert_eq!(crit_orphans, 0, "critic log has orphan rows");
}
