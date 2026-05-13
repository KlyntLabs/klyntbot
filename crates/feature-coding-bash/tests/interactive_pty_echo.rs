//! End-to-end: spawn `bash -c 'read x; echo $x'` with tty=true, send "hello\n"
//! via coding_task_stdin, assert the ring contains the echoed value.

use std::sync::Arc;

use bus::DomainEventBus;
use feature_coding_bash::JobSupervisor;
use klynt_sandbox::MacOsSeatbeltRunner;
use storage::repos::BashJobRepo;
use tools_core::{JobSpec, JobSupervisorHandle};

async fn build_supervisor() -> JobSupervisor {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    let migration = feature_coding_bash::migrations::coding_background_jobs_migration();
    sqlx::query(&migration.sql).execute(pool.inner()).await.unwrap();
    let repo = BashJobRepo::new(pool.inner().clone());
    let bus = Arc::new(DomainEventBus::new(256));
    let queue = Arc::new(bus::context_updates::ContextUpdateQueue::new());
    let data_dir = tempfile::tempdir().unwrap().into_path();
    let sandbox = Arc::new(MacOsSeatbeltRunner::new());
    JobSupervisor::new(repo, bus, queue, data_dir, sandbox)
}

#[tokio::test]
#[cfg(target_os = "macos")]
async fn pty_stdin_round_trip() {
    let sup = build_supervisor().await;
    let spec = JobSpec {
        session_id: "s1".into(),
        agent_id: "a1".into(),
        agent_chain: vec!["a1".into()],
        description: "echo probe".into(),
        command: "read x; echo got=$x".into(),
        cwd: std::env::temp_dir(),
        timeout_ms: 10_000,
        silent_completion: true,
        tty: true,
        tty_rows: Some(24),
        tty_cols: Some(80),
    };
    let view = sup.spawn(spec).await.expect("spawn");
    // Wait for child to call read().
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    sup.write_stdin(&view.id, b"hello\n").await.expect("stdin");
    // Wait for the echo to propagate to the ring.
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    let rd = sup.output_delta(&view.id, 0, false, 0).await.expect("delta");
    let s = String::from_utf8_lossy(&rd.bytes);
    assert!(s.contains("got=hello"), "expected echoed got=hello, got: {s:?}");
}
