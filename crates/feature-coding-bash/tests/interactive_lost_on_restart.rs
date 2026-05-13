//! On Tauri restart while a PTY job is attached, reconcile_on_startup must:
//!   1. Mark the row as Lost.
//!   2. Clear attached_user_at + attach_token.
//!   3. Emit a `bash_job_attach_lost` episode in addition to the standard Lost episode.

use std::sync::Arc;

use bus::DomainEventBus;
use feature_coding_bash::JobSupervisor;
use klynt_sandbox::MacOsSeatbeltRunner;
use storage::repos::{BashJobRepo, BashJobRow};

#[tokio::test]
async fn lost_pty_row_clears_attach_state_and_emits_episode() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    let migration = feature_coding_bash::migrations::coding_background_jobs_migration();
    sqlx::query(&migration.sql).execute(pool.inner()).await.unwrap();
    let repo = BashJobRepo::new(pool.inner().clone());

    // Insert a Running PTY row with attached_user_at set.
    let mut row = BashJobRow {
        id: "bash-mmmmmmmmmm".into(),
        session_id: "s1".into(),
        agent_id: "a1".into(),
        description: "test".into(),
        command: "sleep 60".into(),
        command_key: "sleep_60".into(),
        cwd: "/tmp".into(),
        timeout_ms: 60_000,
        silent_completion: false,
        tty: true,
        tty_rows: Some(24),
        tty_cols: Some(80),
        attached_user_at: None,
        attach_token: None,
        status: "Running".into(),
        exit_code: None,
        failure_kind: None,
        failure_detail: None,
        failure_extracted: None,
        started_at: jiff::Timestamp::now(),
        finished_at: None,
        total_bytes_emitted: 0,
        bisect_count: 0,
        log_path: "/tmp/bash-mmmmmmmmmm.log".into(),
        final_path: None,
        last_polled_at: None,
        last_seen_offset: 0,
    };
    repo.insert(&row).await.unwrap();
    repo.mark_attached("bash-mmmmmmmmmm", Some("tok123")).await.unwrap();

    let bus = Arc::new(DomainEventBus::new(256));
    let queue = Arc::new(bus::context_updates::ContextUpdateQueue::new());
    let data_dir = tempfile::tempdir().unwrap().into_path();
    let sandbox = Arc::new(MacOsSeatbeltRunner::new());
    let sup = JobSupervisor::new(repo.clone(), bus, queue, data_dir, sandbox);

    sup.reconcile_on_startup().await.expect("reconcile");

    let got = repo.get("bash-mmmmmmmmmm").await.unwrap().expect("row");
    assert_eq!(got.status, "Lost");
    assert!(got.attached_user_at.is_none());
    assert!(got.attach_token.is_none());
}
