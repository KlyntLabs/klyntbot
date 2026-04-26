use coding_memory::distiller::retry_queue::{DistillationRetryRepo, RetryReason};
use storage::StoragePool;

#[tokio::test]
async fn enqueue_and_list_due() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();

    let repo = DistillationRetryRepo::new(pool.inner().clone());
    repo.enqueue("s1", Some("t1"), RetryReason::LlmTimeout)
        .await
        .unwrap();

    let due = repo.list_due(10).await.unwrap();
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].session_id, "s1");
    assert_eq!(due[0].attempt_count, 0);
}

#[tokio::test]
async fn record_attempt_backs_off() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();
    let repo = DistillationRetryRepo::new(pool.inner().clone());
    repo.enqueue("s1", Some("t1"), RetryReason::LlmTimeout)
        .await
        .unwrap();
    let id = repo.list_due(10).await.unwrap()[0].id.clone();

    repo.record_attempt(&id).await.unwrap();
    let due = repo.list_due(10).await.unwrap();
    assert_eq!(due.len(), 0); // backed off, not yet due
}

#[tokio::test]
async fn mark_done_removes_entry() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();
    let repo = DistillationRetryRepo::new(pool.inner().clone());
    repo.enqueue("s1", Some("t1"), RetryReason::LlmTimeout)
        .await
        .unwrap();
    let id = repo.list_due(10).await.unwrap()[0].id.clone();
    repo.mark_done(&id).await.unwrap();
    assert_eq!(repo.list_due(10).await.unwrap().len(), 0);
}
