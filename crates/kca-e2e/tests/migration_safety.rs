//! Migration safety — apply all migrations to (a) fresh pool, (b) populated pool.

use storage::StoragePool;

#[tokio::test]
async fn all_migrations_idempotent_on_fresh_pool() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    // First pass: apply all feature migrations.
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();
    let counts_first = list_table_counts(&pool).await;
    // Re-run feature migrations (idempotent by design).
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();
    let counts_second = list_table_counts(&pool).await;
    assert_eq!(counts_first, counts_second, "migrations not idempotent");
}

#[tokio::test]
async fn all_migrations_apply_to_populated_pool() {
    use cognitive::repos::semantic_fact::*;
    let pool = StoragePool::connect_in_memory().await.unwrap();
    // Ensure cognitive tables exist first.
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    let repo = SemanticFactRepo::new(pool.inner().clone());
    for i in 0..50 {
        let fact = cognitive::types::SemanticFact {
            id: format!("S{i}"),
            subject: format!("S{i}"),
            predicate: "p".into(),
            object: "O".into(),
            confidence: 0.5,
            source: "t".into(),
            domain: "test".into(),
            recorded_at: jiff::Timestamp::now().to_string(),
            ..Default::default()
        };
        repo.upsert(&fact).await.unwrap();
    }
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM semantic_facts")
        .fetch_one(pool.inner())
        .await
        .unwrap();
    assert_eq!(count, 50, "rows lost during re-migration");
}

async fn list_table_counts(pool: &StoragePool) -> Vec<(String, i64)> {
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE '_sqlx_%' ORDER BY name"
    ).fetch_all(pool.inner()).await.unwrap();
    let mut out = Vec::new();
    for t in tables {
        let q = format!("SELECT COUNT(*) FROM {}", t);
        let n: i64 = sqlx::query_scalar(&q)
            .fetch_one(pool.inner())
            .await
            .unwrap_or(0);
        out.push((t, n));
    }
    out
}
