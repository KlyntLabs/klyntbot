//! Reconcile-on-startup marks orphans as Lost; preserves .log files.

use std::sync::Arc;

use feature_coding_bash::JobSupervisor;
use storage::StoragePool;
use storage::repos::{BashJobRepo, BashJobRow};
use tempfile::tempdir;


#[tokio::test]
async fn marks_orphan_lost_and_preserves_log() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let migration = feature_coding_bash::migrations::coding_background_jobs_migration();
    StoragePool::run_feature_migrations(pool.inner(), &[migration])
        .await
        .unwrap();

    let dir = tempdir().unwrap();
    let bus = Arc::new(bus::DomainEventBus::new(64));
    let queue = Arc::new(bus::context_updates::ContextUpdateQueue::new());
    let sandbox = Arc::new(klynt_sandbox::MacOsSeatbeltRunner::new());
    let repo = BashJobRepo::new(pool.inner().clone());

    // Insert a fake "Running" row + create a fake .log file
    let log_path = dir.path().join("jobs").join("bash-0rphan001a.log");
    tokio::fs::create_dir_all(log_path.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::write(&log_path, b"partial output\n")
        .await
        .unwrap();
    repo.insert(&BashJobRow {
        id: "bash-0rphan001a".into(),
        session_id: "s1".into(),
        agent_id: "root".into(),
        description: "orphan".into(),
        command: "sleep 999".into(),
        cwd: dir.path().to_string_lossy().to_string(),
        timeout_ms: 600_000,
        silent_completion: false,
        status: "Running".into(),
        exit_code: None,
        failure_kind: None,
        failure_detail: None,
        failure_extracted: None,
        started_at: jiff::Timestamp::now(),
        finished_at: None,
        total_bytes_emitted: 16,
        bisect_count: 0,
        log_path: log_path.to_string_lossy().to_string(),
        final_path: None,
        last_polled_at: None,
        last_seen_offset: 0,
    })
    .await
    .unwrap();

    let supervisor = Arc::new(JobSupervisor::new(
        repo.clone(),
        bus,
        queue.clone(),
        dir.path().to_path_buf(),
        sandbox,
    ));

    let count = supervisor.reconcile_on_startup().await.unwrap();
    assert_eq!(count, 1);

    let row = repo.get("bash-0rphan001a").await.unwrap().unwrap();
    assert_eq!(row.status, "Lost");
    assert_eq!(row.failure_kind.unwrap(), "Lost");

    // .log preserved
    assert!(log_path.exists(), "log should be preserved");

    // ContextUpdate enqueued
    let updates = queue.drain();
    assert!(updates.iter().any(|u| u
        .content
        .as_deref()
        .map(|c| c.contains("lost"))
        .unwrap_or(false)));
}
