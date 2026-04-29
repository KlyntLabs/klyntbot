#![cfg(feature = "soak")]

use kca_e2e::fixtures::*;
use kca_e2e::replayer::ReplayContext;

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
#[ignore = "soak test is feature-gated and long-running"]
async fn soak_10k_turns_memory_stable() {
    kca_e2e::init_test_logging();
    let path = fixtures_root().join("soak_10k.jsonl");
    let fixtures: Vec<ConversationFixture> = load_jsonl(&path).expect("load soak");
    assert!(fixtures.len() >= 100, "expected ≥100 base fixtures");
    let mut ctx = ReplayContext::new().await.unwrap();
    let target = 10_000;
    let mut completed = 0;
    let mut sample: Vec<(usize, usize)> = vec![];
    'outer: loop {
        for f in &fixtures {
            ctx.replay(f).await.unwrap();
            completed += f.turns.len();
            if completed % 1000 == 0 {
                let n: i64 = sqlx::query_scalar(
                    "SELECT COUNT(*) FROM semantic_facts WHERE valid_until IS NULL",
                )
                .fetch_one(ctx.pool.inner())
                .await
                .unwrap();
                sample.push((completed, n as usize));
                tracing::info!(turns = completed, facts = n, "soak progress");
            }
            if completed >= target {
                break 'outer;
            }
        }
    }
    let early = sample
        .iter()
        .find(|(t, _)| *t == 2000)
        .map(|(_, n)| *n)
        .unwrap_or(1);
    let late = sample.last().map(|(_, n)| *n).unwrap_or(1);
    assert!(
        late < early * 5,
        "fact count grows super-linearly: 2k={early}, 10k={late}"
    );
}
