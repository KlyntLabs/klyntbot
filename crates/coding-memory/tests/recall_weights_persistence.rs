use coding_memory::recall::{load_recall_weights, store_recall_weights};
use storage::StoragePool;

#[tokio::test]
async fn recall_weights_round_trip() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();

    let custom = [0.4, 0.05, 0.1, 0.05, 0.05, 0.15, 0.05, 0.05, 0.04, 0.04, 0.02, 0.0];
    store_recall_weights(&pool, &custom).await.unwrap();
    let loaded = load_recall_weights(&pool).await.unwrap();
    assert_eq!(loaded, custom);
}

#[tokio::test]
async fn recall_weights_default_when_unset() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();

    let loaded = load_recall_weights(&pool).await.unwrap();
    let default = coding_memory::recall::default_weights();
    assert_eq!(loaded, default);
}
