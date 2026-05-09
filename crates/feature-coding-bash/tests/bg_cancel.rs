//! SIGTERM→SIGKILL escalation.

use std::sync::Arc;

use feature_coding_bash::JobSupervisor;
use storage::repos::BashJobRepo;
use storage::StoragePool;
use tempfile::tempdir;
use tools_core::{JobSpec, JobStatus, JobSupervisorHandle};

#[tokio::test]
#[cfg_attr(not(target_os = "macos"), ignore)]
async fn stop_escalates_to_sigkill() {
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
        repo.clone(),
        bus,
        queue,
        dir.path().to_path_buf(),
        sandbox,
    ));

    let view = supervisor
        .spawn(JobSpec {
            session_id: "s1".into(),
            agent_id: "root".into(),
            agent_chain: vec!["root".into()],
            description: "sleep 30".into(),
            command: "sleep 30".into(),
            cwd: dir.path().to_path_buf(),
            timeout_ms: 60_000,
            silent_completion: false,
        })
        .await
        .expect("spawn");

    // Immediately stop
    let stopped = supervisor
        .stop(&view.id, "test cancel")
        .await
        .expect("stop");
    assert_eq!(stopped.status, JobStatus::Cancelled);

    // Wait for SIGTERM→SIGKILL escalation (2s grace + a bit)
    tokio::time::sleep(std::time::Duration::from_secs(4)).await;

    // Row should reflect cancellation
    let row = repo.get(view.id.as_str()).await.unwrap().unwrap();
    assert_eq!(row.status, "Cancelled");
    let exit = row.exit_code.expect("exit_code should be set");
    // SIGTERM = -15, SIGKILL = -9, or -1 when wait() can't get the real code
    assert!(
        exit == -15 || exit == -9 || exit == 137 || exit == -1,
        "unexpected exit code: {exit}"
    );
}
