//! reap_session kills processes without deleting rows.

use std::sync::Arc;

use feature_coding_bash::JobSupervisor;
use storage::repos::BashJobRepo;
use storage::StoragePool;
use tempfile::tempdir;
use tools_core::{JobSpec, JobSupervisorHandle};

#[tokio::test]
#[cfg_attr(not(target_os = "macos"), ignore)]
async fn reap_kills_jobs_leaves_rows() {
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

    // Spawn 2 jobs in session A
    for i in 0..2 {
        supervisor
            .spawn(JobSpec {
                session_id: "session-a".into(),
                agent_id: "root".into(),
                agent_chain: vec!["root".into()],
                description: format!("job-{i}"),
                command: "sleep 30".into(),
                cwd: dir.path().to_path_buf(),
                timeout_ms: 60_000,
                silent_completion: false,
            })
            .await
            .expect("spawn");
    }

    let reaped = supervisor.reap_session("session-a").await.unwrap();
    assert_eq!(reaped, 2);

    // Give handle_exit time to run
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // Rows should still exist (cascade-delete is by FK; reap doesn't delete rows)
    let all = repo.list_all_for_session("session-a", false).await.unwrap();
    assert_eq!(all.len(), 2);

    // Both should be terminal
    for row in &all {
        assert!(
            row.status == "Cancelled" || row.status == "Lost",
            "expected terminal status, got {}",
            row.status
        );
    }
}
