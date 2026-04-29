//! Full-pipeline E2E: replay a multi-turn fixture through AppCore and assert gates.

use kca_e2e::asserts::*;
use kca_e2e::fixtures::{fixtures_root, load_jsonl, ConversationFixture};
use kca_e2e::replayer::ReplayContext;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn full_pipeline_longmembench_subset() {
    kca_e2e::init_test_logging();
    let path = fixtures_root().join("longmembench_subset.jsonl");
    let fixtures: Vec<ConversationFixture> = load_jsonl(&path).expect("load fixtures");
    assert!(!fixtures.is_empty(), "fixture file empty");

    let limit = std::env::var("KCA_E2E_LIMIT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3);
    let mut ctx = ReplayContext::new().await.unwrap();
    let mut total = kca_e2e::replayer::ReplayMeasurements::default();
    for f in fixtures.iter().take(limit) {
        let m = ctx.replay(f).await.unwrap();
        total.turns_replayed += m.turns_replayed;
        total.turn_latencies_ms.extend(m.turn_latencies_ms);
    }

    let mut gates = vec![
        assert_f1_fact_to_edge_ratio(&ctx, 0.6).await,
        assert_p1_p95_latency(&total, 400),
    ];
    gates.push(assert_f4_critic_catches_hallucinations(&ctx, &fixtures, 0.95).await);

    let report = render_gate_table(&gates);
    println!("\n=== Full pipeline ===\n{report}");

    for g in &gates {
        assert!(g.passed, "GATE {} FAILED: {}", g.gate_id, g.message);
    }
}
