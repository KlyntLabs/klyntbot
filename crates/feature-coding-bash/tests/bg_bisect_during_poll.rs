//! Bisect-on-overflow: bisect_occurred_since=true on subsequent poll.

use std::sync::Arc;

use feature_coding_bash::JobSupervisor;
use storage::repos::BashJobRepo;
use storage::StoragePool;
use tempfile::tempdir;
use tools_core::{JobSpec, JobSupervisorHandle};

#[tokio::test]
#[cfg_attr(not(target_os = "macos"), ignore)]
async fn bisect_flag_set_on_overflow() {
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
        queue,
        dir.path().to_path_buf(),
        sandbox,
    ));

    // Spawn a script that produces ~5 MB of output quickly
    let view = supervisor
        .spawn(JobSpec {
            session_id: "s1".into(),
            agent_id: "root".into(),
            agent_chain: vec!["root".into()],
            description: "massive output".into(),
            command: r#"yes "hello world this is a test string" | head -c 5000000"#.into(),
            cwd: dir.path().to_path_buf(),
            timeout_ms: 60_000,
            silent_completion: false,
            tty: false,
            tty_rows: None,
            tty_cols: None,
        })
        .await
        .expect("spawn");

    // Poll repeatedly while the job is still running; we need to catch it
    // after total_bytes crosses the 4 MB ring cap but before handle_exit
    // removes it from the live registry.
    let mut found = false;
    for _ in 0..100 {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        let rd = supervisor
            .output_delta(&view.id, 0, false, 0)
            .await
            .unwrap();
        if rd.bisect_occurred_since {
            found = true;
            break;
        }
    }

    assert!(
        found,
        "expected bisect_occurred_since=true while job was live"
    );

    // Let handle_exit finish
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
}
