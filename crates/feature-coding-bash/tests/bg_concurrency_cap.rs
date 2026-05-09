//! Cap = 6 active jobs per (session, agent_chain).

use std::sync::Arc;

use feature_coding_bash::JobSupervisor;
use storage::repos::BashJobRepo;
use storage::StoragePool;
use tempfile::tempdir;
use tools_core::{JobError, JobSpec, JobSupervisorHandle};

#[tokio::test]
#[cfg_attr(not(target_os = "macos"), ignore)]
async fn cap_rejected_at_seven() {
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

    let supervisor = Arc::new(JobSupervisor::new(
        repo,
        bus,
        queue,
        dir.path().to_path_buf(),
        sandbox,
    ));

    let mk = |i: usize| JobSpec {
        session_id: "s1".into(),
        agent_id: "root".into(),
        agent_chain: vec!["root".into()],
        description: format!("job-{i}"),
        command: "sleep 30".into(),
        cwd: dir.path().to_path_buf(),
        timeout_ms: 60_000,
        silent_completion: false,
    };

    // 6 should succeed
    for i in 0..6 {
        supervisor
            .spawn(mk(i))
            .await
            .expect("first 6 should succeed");
    }
    // 7th rejected
    let err = supervisor.spawn(mk(6)).await.expect_err("7th should fail");
    assert!(matches!(err, JobError::CapReached { .. }));
}
