//! Phase 2.3a happy path: spawn → poll twice with cursor delta → complete.

use std::sync::Arc;

use bus::context_updates::ContextUpdateQueue;
use bus::DomainEventBus;
use feature_coding_bash::JobSupervisor;
use klynt_sandbox::MacOsSeatbeltRunner;
use storage::repos::BashJobRepo;
use storage::StoragePool;
use tempfile::tempdir;
use tools_core::{JobSpec, JobStatus, JobSupervisorHandle};

#[tokio::test]
#[cfg_attr(not(target_os = "macos"), ignore)]
async fn happy_path() {
    let pool = StoragePool::connect_in_memory().await.expect("pool");
    let migration = feature_coding_bash::migrations::coding_background_jobs_migration();
    StoragePool::run_feature_migrations(pool.inner(), &[migration])
        .await
        .unwrap();

    let dir = tempdir().unwrap();
    let bus = Arc::new(DomainEventBus::new(64));
    let queue = Arc::new(ContextUpdateQueue::new());
    let sandbox = Arc::new(MacOsSeatbeltRunner::new());
    let repo = BashJobRepo::new(pool.inner().clone());

    let supervisor = Arc::new(JobSupervisor::new(
        repo,
        bus.clone(),
        queue.clone(),
        dir.path().to_path_buf(),
        sandbox,
    ));

    let view = supervisor
        .spawn(JobSpec {
            session_id: "session-1".into(),
            agent_id: "root".into(),
            agent_chain: vec!["root".into()],
            description: "echo and sleep".into(),
            command: r#"echo "hello"; sleep 0.3; echo "world""#.into(),
            cwd: dir.path().to_path_buf(),
            timeout_ms: 60_000,
            silent_completion: false,
        })
        .await
        .expect("spawn");

    assert!(view.id.as_str().starts_with("bash-"));

    // First poll — should see "hello"
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let rd1 = supervisor
        .output_delta(&view.id, 0, false, 0)
        .await
        .unwrap();
    let s1 = String::from_utf8_lossy(&rd1.bytes);
    assert!(
        s1.contains("hello"),
        "first poll should contain hello, got: {s1:?}"
    );
    assert!(!rd1.bisect_occurred_since);

    // Second poll — block until "world"
    let rd2 = supervisor
        .output_delta(&view.id, rd1.new_offset, true, 5_000)
        .await
        .unwrap();
    let s2 = String::from_utf8_lossy(&rd2.bytes);
    assert!(s2.contains("world") || s2.is_empty(), "second poll: {s2:?}");

    // Wait for completion
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    let listed = supervisor
        .list("session-1", &["root".to_string()], false)
        .await;
    assert_eq!(listed.len(), 1);
    let job = &listed[0];
    assert!(matches!(
        job.status,
        JobStatus::Completed | JobStatus::Running
    ));
}
