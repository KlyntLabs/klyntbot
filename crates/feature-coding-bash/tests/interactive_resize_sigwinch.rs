//! Verify that resize() reaches the child via SIGWINCH. The probe shell prints
//! its current LINES/COLUMNS twice — once before resize, once after.

use std::sync::Arc;

use bus::DomainEventBus;
use feature_coding_bash::JobSupervisor;
use klynt_sandbox::MacOsSeatbeltRunner;
use storage::repos::BashJobRepo;
use tools_core::{JobSpec, JobSupervisorHandle};

#[tokio::test]
#[cfg(target_os = "macos")]
async fn pty_resize_updates_child_terminal_size() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    let migration = feature_coding_bash::migrations::coding_background_jobs_migration();
    sqlx::query(&migration.sql).execute(pool.inner()).await.unwrap();
    let sup = JobSupervisor::new(
        BashJobRepo::new(pool.inner().clone()),
        Arc::new(DomainEventBus::new(256)),
        Arc::new(bus::context_updates::ContextUpdateQueue::new()),
        tempfile::tempdir().unwrap().into_path(),
        Arc::new(MacOsSeatbeltRunner::new()),
    );
    let spec = JobSpec {
        session_id: "s1".into(),
        agent_id: "a1".into(),
        agent_chain: vec!["a1".into()],
        description: "resize probe".into(),
        command: "stty size; sleep 0.5; stty size".into(),
        cwd: std::env::temp_dir(),
        timeout_ms: 10_000,
        silent_completion: true,
        tty: true,
        tty_rows: Some(24),
        tty_cols: Some(80),
    };
    let view = sup.spawn(spec).await.expect("spawn");
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    sup.resize(&view.id, 30, 120).await.expect("resize");
    tokio::time::sleep(std::time::Duration::from_millis(800)).await;
    let rd = sup.output_delta(&view.id, 0, false, 0).await.unwrap();
    let s = String::from_utf8_lossy(&rd.bytes);
    // After resize, the second `stty size` line should print 30 120.
    assert!(s.contains("30 120"), "expected 30 120 in output, got: {s:?}");
}
