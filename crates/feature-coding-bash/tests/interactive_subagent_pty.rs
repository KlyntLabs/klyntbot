use std::sync::Arc;

use bus::DomainEventBus;
use feature_coding_bash::JobSupervisor;
use klynt_sandbox::MacOsSeatbeltRunner;
use storage::repos::BashJobRepo;
use tools_core::{JobSpec, JobSupervisorHandle};

#[tokio::test]
#[cfg(target_os = "macos")]
async fn subagent_pty_job_carries_agent_id_through_to_row() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    sqlx::query(&feature_coding_bash::migrations::coding_background_jobs_migration().sql)
        .execute(pool.inner())
        .await
        .unwrap();
    let repo = BashJobRepo::new(pool.inner().clone());
    let sup = JobSupervisor::new(
        repo.clone(),
        Arc::new(DomainEventBus::new(256)),
        Arc::new(bus::context_updates::ContextUpdateQueue::new()),
        tempfile::tempdir().unwrap().into_path(),
        Arc::new(MacOsSeatbeltRunner::new()),
    );
    let view = sup
        .spawn(JobSpec {
            session_id: "s1".into(),
            agent_id: "subagent-X".into(),
            agent_chain: vec!["root".into(), "subagent-X".into()],
            description: "child probe".into(),
            command: "sleep 5".into(),
            cwd: std::env::temp_dir(),
            timeout_ms: 30_000,
            silent_completion: true,
            tty: true,
            tty_rows: Some(24),
            tty_cols: Some(80),
        })
        .await
        .expect("spawn");
    let row = repo.get(view.id.as_str()).await.unwrap().unwrap();
    assert_eq!(row.agent_id, "subagent-X");
    assert!(row.tty);
    let _ = sup.stop(&view.id, "cleanup").await;
}
