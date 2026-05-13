//! agent_chain visibility: parent jobs visible to subagents.

use std::sync::Arc;

use feature_coding_bash::JobSupervisor;
use storage::repos::BashJobRepo;
use storage::StoragePool;
use tempfile::tempdir;
use tools_core::{JobSpec, JobSupervisorHandle};

#[tokio::test]
#[cfg_attr(not(target_os = "macos"), ignore)]
async fn parent_job_visible_to_subagent_chain() {
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

    // Spawn job A as agent_id="root"
    supervisor
        .spawn(JobSpec {
            session_id: "s1".into(),
            agent_id: "root".into(),
            agent_chain: vec!["root".into()],
            description: "parent job".into(),
            command: "sleep 30".into(),
            cwd: dir.path().to_path_buf(),
            timeout_ms: 60_000,
            silent_completion: false,
            tty: false,
            tty_rows: None,
            tty_cols: None,
        })
        .await
        .expect("spawn");

    // list with chain ["root", "subagent-1"] should still see it
    let visible = supervisor
        .list("s1", &["root".into(), "subagent-1".into()], true)
        .await;
    assert_eq!(visible.len(), 1);

    // count_active_for_chain should return 1
    let count = repo
        .count_active_for_chain("s1", &["root".into(), "subagent-1".into()])
        .await
        .unwrap();
    assert_eq!(count, 1);
}
