use cognitive::repos::semantic_fact::SemanticFactRepo;
use cognitive::types::SemanticFact;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use storage::StoragePool;

fn bench_extraction_write(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    c.bench_function("semantic_fact_upsert_100", |b| {
        b.to_async(&rt).iter(|| async {
            let pool = StoragePool::connect_in_memory().await.unwrap();
            StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
                .await
                .unwrap();
            let repo = SemanticFactRepo::new(pool.inner().clone());
            for i in 0..100 {
                let f = SemanticFact {
                    id: format!("S{i}"),
                    subject: format!("S{i}"),
                    predicate: "p".into(),
                    object: format!("O{i}"),
                    confidence: 0.5,
                    source: "t".into(),
                    domain: "bench".into(),
                    recorded_at: jiff::Timestamp::now().to_string(),
                    ..Default::default()
                };
                repo.upsert(black_box(&f)).await.unwrap();
            }
        });
    });
}

criterion_group!(benches, bench_extraction_write);
criterion_main!(benches);
