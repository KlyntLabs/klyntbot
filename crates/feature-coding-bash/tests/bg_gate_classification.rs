//! Per-FailureKind classification with real fixture outputs.

use std::sync::Arc;

use feature_coding_bash::JobSupervisor;
use storage::repos::BashJobRepo;
use storage::StoragePool;
use tempfile::tempdir;
use tools_core::{JobSpec, JobSupervisorHandle};

async fn run_script(
    supervisor: &JobSupervisor,
    description: &str,
    script: &str,
) -> tools_core::JobView {
    let dir = std::env::temp_dir();
    supervisor
        .spawn(JobSpec {
            session_id: "s1".into(),
            agent_id: "root".into(),
            agent_chain: vec!["root".into()],
            description: description.into(),
            command: script.into(),
            cwd: dir,
            timeout_ms: 60_000,
            silent_completion: false,
            tty: false,
            tty_rows: None,
            tty_cols: None,
        })
        .await
        .expect("spawn")
}

#[tokio::test]
#[cfg_attr(not(target_os = "macos"), ignore)]
async fn compile_error_classified() {
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

    // Produce a Rust compile error
    let view = run_script(
        &supervisor,
        "compile error",
        r#"rustc --edition 2021 - <<'EOF'
fn main() { let x: String = 42; }
EOF"#,
    )
    .await;

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let row = repo.get(view.id.as_str()).await.unwrap().unwrap();
    assert_eq!(row.status, "Failed");
    let kind = row.failure_kind.expect("failure_kind should be set");
    assert!(
        kind.contains("CompileError") || kind.contains("Other"),
        "expected CompileError-ish, got {kind}"
    );
}

#[tokio::test]
#[cfg_attr(not(target_os = "macos"), ignore)]
async fn test_failure_classified() {
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

    // Produce a test failure
    let view = run_script(
        &supervisor,
        "test failure",
        r#"echo "running 2 tests"; echo "test one ... FAILED"; echo "test two ... ok"; echo "failures:"; echo "    one"; exit 1"#,
    )
    .await;

    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let row = repo.get(view.id.as_str()).await.unwrap().unwrap();
    assert_eq!(row.status, "Failed");
    let kind = row.failure_kind.expect("failure_kind should be set");
    assert!(
        kind.contains("TestFailure") || kind.contains("Other"),
        "expected TestFailure-ish, got {kind}"
    );
}

#[tokio::test]
#[cfg_attr(not(target_os = "macos"), ignore)]
async fn eaddrinuse_classified() {
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

    // Produce EADDRINUSE
    let view = run_script(
        &supervisor,
        "port conflict",
        r#"echo "Error: listen EADDRINUSE: address already in use :::3000" >&2; exit 1"#,
    )
    .await;

    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let row = repo.get(view.id.as_str()).await.unwrap().unwrap();
    assert_eq!(row.status, "Failed");
    let kind = row.failure_kind.expect("failure_kind should be set");
    assert!(
        kind.contains("NetworkBindFailure") || kind.contains("Other"),
        "expected NetworkBindFailure-ish, got {kind}"
    );
}
