use coding_memory::reforge::selective_delete::{SelectiveDeleteLogRepo, SelectiveDeleteSignal};
use storage::StoragePool;

#[tokio::test]
async fn halves_stability_on_uncited_memories() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();

    // Seed a fact with stability 4.0.
    sqlx::query(
        "INSERT INTO semantic_facts \
         (id, subject, predicate, object, confidence, domain, scope_type, scope_id, \
          valid_from, memory_type, stability, access_count) \
         VALUES ('f1','s','p','o',0.9,'work','code',NULL,?1,'fact',4.0,0)",
    )
    .bind(jiff::Timestamp::now().to_string())
    .execute(pool.inner())
    .await
    .unwrap();

    // Seed 6 retrievals with cited_in_response = false.
    for i in 0..6 {
        sqlx::query(
            "INSERT INTO memory_utilization (id, memory_id, cited_in_response) \
             VALUES (?1, 'f1', 0)",
        )
        .bind(format!("u{i}"))
        .execute(pool.inner())
        .await
        .unwrap();
    }

    let log = SelectiveDeleteLogRepo::new(pool.clone());
    let count = SelectiveDeleteSignal::apply_with_threshold(&pool, &log, 5)
        .await
        .expect("apply");
    assert!(count >= 1);

    let new_stability: (f32,) =
        sqlx::query_as("SELECT stability FROM semantic_facts WHERE id = 'f1'")
            .fetch_one(pool.inner())
            .await
            .unwrap();
    assert!((new_stability.0 - 2.0).abs() < 1e-3);

    let log_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM selective_delete_log WHERE memory_id = 'f1'")
            .fetch_one(pool.inner())
            .await
            .unwrap();
    assert_eq!(log_count.0, 1);
}
