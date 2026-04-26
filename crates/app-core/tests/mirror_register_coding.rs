use app_core::coding_memory::mirror::register_coding_sources;
use storage::StoragePool;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn registers_4_coding_sources_without_error() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();

    let mirror_repo = cognitive::mirror::MirrorRepo::new(pool.clone());
    let shutdown = CancellationToken::new();
    let registered = register_coding_sources(pool, mirror_repo, shutdown.clone())
        .await
        .unwrap();
    assert_eq!(registered.consumers.len(), 4);

    shutdown.cancel();
    for h in registered.flush_handles {
        let _ = h.await;
    }
}
