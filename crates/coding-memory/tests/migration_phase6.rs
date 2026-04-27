//! Phase-6 migration applies cleanly and adds the new table + index.

use storage::StoragePool;

#[tokio::test]
async fn migration_005_creates_pending_invalidations() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();

    let exists: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='pending_invalidations'",
    )
    .fetch_one(pool.inner())
    .await
    .unwrap();
    assert_eq!(exists, 1, "pending_invalidations table missing");

    let row_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pending_invalidations")
        .fetch_one(pool.inner())
        .await
        .unwrap();
    assert_eq!(row_count, 0);
}

#[tokio::test]
async fn migration_005_creates_anchored_symbol_index() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();

    let names: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type='index' AND tbl_name='semantic_facts'",
    )
    .fetch_all(pool.inner())
    .await
    .unwrap();
    assert!(
        names.iter().any(|(n,)| n == "idx_anchored_symbol_file_facts"),
        "expected functional anchored-symbol index on semantic_facts; got {names:?}"
    );
}
