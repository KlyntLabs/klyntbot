use std::sync::Arc;

use bus::DomainEventBus;
use feature_coding_bash::JobSupervisor;
use klynt_sandbox::MacOsSeatbeltRunner;
use storage::repos::BashJobRepo;
use tools_core::{AttachError, JobSpec, JobSupervisorHandle};

#[tokio::test]
#[cfg(target_os = "macos")]
async fn second_attach_returns_already_attached() {
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
            description: "x".into(),
            command: "sleep 10".into(),
            cwd: std::env::temp_dir(),
            timeout_ms: 30_000,
            silent_completion: true,
            tty: true,
            tty_rows: Some(24),
            tty_cols: Some(80),
        })
        .await
        .expect("spawn");
    let _a1 = sup.attach(&view.id).await.expect("first attach");
    let err = sup.attach(&view.id).await;
    assert!(matches!(err, Err(AttachError::AlreadyAttached)));
    let _ = sup.stop(&view.id, "cleanup").await;
}
