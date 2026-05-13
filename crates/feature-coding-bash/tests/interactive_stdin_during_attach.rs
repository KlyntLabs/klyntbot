//! Spawn + attach + LLM calls coding_task_stdin while user is attached.
//! Verify both writes appear in the ring (interleaved at byte level).

use std::sync::Arc;

use bus::DomainEventBus;
use feature_coding_bash::JobSupervisor;
use klynt_sandbox::MacOsSeatbeltRunner;
use storage::repos::BashJobRepo;
use tools_core::{JobSpec, JobSupervisorHandle};

#[tokio::test]
#[cfg(target_os = "macos")]
async fn dual_stdin_writes_both_reach_pty() {
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
            description: "dual stdin".into(),
            command: "read a; read b; echo a=$a; echo b=$b".into(),
            cwd: std::env::temp_dir(),
            timeout_ms: 10_000,
            silent_completion: true,
            tty: true,
            tty_rows: Some(24),
            tty_cols: Some(80),
        })
        .await
        .expect("spawn");
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    // Two writes — represent LLM (write_stdin) and user (also write_stdin via the
    // same path, since attach hasn't wired a separate channel in this unit test).
    sup.write_stdin(&view.id, b"first\n").await.expect("w1");
    sup.write_stdin(&view.id, b"second\n").await.expect("w2");
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    let rd = sup.output_delta(&view.id, 0, false, 0).await.unwrap();
    let s = String::from_utf8_lossy(&rd.bytes);
    assert!(s.contains("a=first"), "expected a=first, got: {s:?}");
    assert!(s.contains("b=second"), "expected b=second, got: {s:?}");
}
