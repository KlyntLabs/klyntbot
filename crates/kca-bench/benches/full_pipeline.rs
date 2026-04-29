use criterion::{black_box, criterion_group, criterion_main, Criterion};
use kca_e2e::fixtures::*;

fn bench_full_pipeline_stub(c: &mut Criterion) {
    let fixture = sample_fixture();
    c.bench_function("full_pipeline_stub", |b| {
        b.iter(|| {
            let _ = black_box(&fixture);
        });
    });
}

fn sample_fixture() -> ConversationFixture {
    ConversationFixture {
        id: "bench".into(),
        source: "bench".into(),
        turns: vec![TurnFixture {
            user: "Alice works at Anthropic".into(),
            assistant: "Got it.".into(),
            tool_calls: vec![],
            ground_truth_facts: vec![],
            cli_source: None,
            recorded_at: None,
        }],
        queries: vec![],
        metadata: serde_json::Value::Null,
    }
}

criterion_group!(benches, bench_full_pipeline_stub);
criterion_main!(benches);
