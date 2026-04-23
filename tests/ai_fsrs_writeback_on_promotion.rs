//! Integration test: promoted trial params update `fsrs_parameters.desired_retention`.
//!
//! Exercises the `agent::adapters::fsrs_writeback::apply_fsrs_writeback` bridge
//! against a real in-memory pool with cognitive migrations applied.

#[tokio::test]
async fn promotion_writes_desired_retention_to_table() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();

    // Apply cognitive migrations so the fsrs_parameters table + seed row exist.
    let migrations = klyntbot::cognitive::repos::cognitive_migrations();
    storage::StoragePool::run_feature_migrations(pool.inner(), &migrations)
        .await
        .unwrap();

    let repo = klyntbot::cognitive::FsrsParamsRepo::new(pool.clone());

    let trial = common::TrialParams {
        fsrs_desired_retention: Some(0.88),
        ..Default::default()
    };

    klyntbot::agent::adapters::fsrs_writeback::apply_fsrs_writeback(&repo, &trial)
        .await
        .expect("writeback ok");

    let (_w, ret): (String, f64) =
        sqlx::query_as("SELECT weights, desired_retention FROM fsrs_parameters WHERE id = 'local'")
            .fetch_one(pool.inner())
            .await
            .unwrap();

    assert!((ret - 0.88).abs() < 1e-9, "expected 0.88, got {ret}");
}

#[tokio::test]
async fn promotion_with_none_retention_is_no_op() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();

    let migrations = klyntbot::cognitive::repos::cognitive_migrations();
    storage::StoragePool::run_feature_migrations(pool.inner(), &migrations)
        .await
        .unwrap();

    let repo = klyntbot::cognitive::FsrsParamsRepo::new(pool.clone());

    // Seed the table with a known value first.
    repo.update_desired_retention(0.80).await.unwrap();

    // Apply writeback with no fsrs_desired_retention set.
    let trial = common::TrialParams {
        fsrs_desired_retention: None,
        ..Default::default()
    };
    klyntbot::agent::adapters::fsrs_writeback::apply_fsrs_writeback(&repo, &trial)
        .await
        .expect("no-op writeback ok");

    let (_w, ret): (String, f64) =
        sqlx::query_as("SELECT weights, desired_retention FROM fsrs_parameters WHERE id = 'local'")
            .fetch_one(pool.inner())
            .await
            .unwrap();

    assert!(
        (ret - 0.80).abs() < 1e-9,
        "expected 0.80 unchanged, got {ret}"
    );
}
