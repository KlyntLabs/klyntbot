use std::sync::Arc;
use std::time::{Duration, Instant};

use bus::DomainEventBus;
use feature_coding_bash::JobSupervisor;
use klynt_sandbox::MacOsSeatbeltRunner;
use storage::repos::BashJobRepo;
use tools_core::{JobSpec, JobStatus, JobSupervisorHandle};

#[tokio::test]
#[cfg(target_os = "macos")]
async fn stopping_pty_job_cancels_within_2s() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    sqlx::query(&feature_coding_bash::migrations::coding_background_jobs_migration().sql)
        .execute(pool.inner())
        .await
        .unwrap();
    let sup = JobSupervisor::new(
        BashJobRepo::new(pool.inner().clone()),
        Arc::new(DomainEventBus::new(256)),
        Arc::new(bus::context_updates::ContextUpdateQueue::new()),
        tempfile::tempdir().unwrap().into_path(),
        Arc::new(MacOsSeatbeltRunner::new()),
    );
    let view = sup
        .spawn(JobSpec {
            session_id: "s1".into(),
            agent_id: "a1".into(),
            agent_chain: vec!["a1".into()],
            description: "long sleep".into(),
            command: "sleep 60".into(),
            cwd: std::env::temp_dir(),
            timeout_ms: 120_000,
            silent_completion: true,
            tty: true,
            tty_rows: Some(24),
            tty_cols: Some(80),
        })
        .await
        .expect("spawn");
    let start = Instant::now();
    let stopped = sup.stop(&view.id, "test").await.expect("stop");
    assert_eq!(stopped.status, JobStatus::Cancelled);
    assert!(start.elapsed() < Duration::from_secs(3), "stop should be fast");
}
