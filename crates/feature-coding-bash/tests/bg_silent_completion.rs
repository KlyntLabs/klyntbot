//! silent_completion=true skips the ContextUpdate inject.

use std::sync::Arc;

use bus::context_updates::ContextUpdateReason;
use feature_coding_bash::JobSupervisor;
use storage::repos::BashJobRepo;
use storage::StoragePool;
use tempfile::tempdir;
use tools_core::{JobSpec, JobSupervisorHandle};

#[tokio::test]
#[cfg_attr(not(target_os = "macos"), ignore)]
async fn silent_completion_skips_push() {
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

    // Drain any pre-existing updates (like the Started update)
    let _ = queue.drain();

    supervisor
        .spawn(JobSpec {
            session_id: "s1".into(),
            agent_id: "root".into(),
            agent_chain: vec!["root".into()],
            description: "silent fail".into(),
            command: "false".into(),
            cwd: dir.path().to_path_buf(),
            timeout_ms: 60_000,
            silent_completion: true,
        })
        .await
        .expect("spawn");

    // Wait for handle_exit to complete
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let updates = queue.drain();
    // Should NOT contain a Failed/Completed update
    assert!(
        !updates
            .iter()
            .any(|u| u.reason == ContextUpdateReason::CodingJobsChanged
                && u.content
                    .as_deref()
                    .map(|c| c.contains("Failed") || c.contains("Completed"))
                    .unwrap_or(false)),
        "expected NO Failed/Completed CodingJobsChanged update, got: {:?}",
        updates
    );
}
