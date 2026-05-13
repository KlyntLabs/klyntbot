//! Auto-injected ContextUpdate on non-silent completion.

use std::sync::Arc;

use bus::context_updates::ContextUpdateReason;
use feature_coding_bash::JobSupervisor;
use storage::repos::BashJobRepo;
use storage::StoragePool;
use tempfile::tempdir;
use tools_core::{JobSpec, JobSupervisorHandle};

#[tokio::test]
#[cfg_attr(not(target_os = "macos"), ignore)]
async fn failed_job_pushes_context_update() {
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
        queue.clone(),
        dir.path().to_path_buf(),
        sandbox,
    ));

    supervisor
        .spawn(JobSpec {
            session_id: "s1".into(),
            agent_id: "root".into(),
            agent_chain: vec!["root".into()],
            description: "immediate fail".into(),
            command: "false".into(),
            cwd: dir.path().to_path_buf(),
            timeout_ms: 60_000,
            silent_completion: false,
            tty: false,
            tty_rows: None,
            tty_cols: None,
        })
        .await
        .expect("spawn");

    // Wait for handle_exit to complete
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let updates = queue.drain();
    assert!(
        updates
            .iter()
            .any(|u| u.reason == ContextUpdateReason::CodingJobsChanged
                && u.content
                    .as_deref()
                    .map(|c| c.contains("Failed"))
                    .unwrap_or(false)),
        "expected CodingJobsChanged update with 'Failed', got: {:?}",
        updates
    );
}
