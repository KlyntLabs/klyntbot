//! Phase 2.3b integration: completion body contains a diff section when a
//! prior run with the same command_key exists.

use std::sync::Arc;
use std::time::Duration;

use bus::{ContextUpdateQueue, DomainEventBus};
use feature_coding_bash::JobSupervisor;
use storage::{repos::BashJobRepo, StoragePool};
use tools_core::{JobSpec, JobSupervisorHandle};

async fn pool_with_table() -> StoragePool {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    // Apply just the bash jobs migration manually.
    let migration = feature_coding_bash::migrations::coding_background_jobs_migration();
    sqlx::query(&migration.sql).execute(pool.inner()).await.unwrap();
    pool
}

fn spec(command: &str) -> JobSpec {
    JobSpec {
        session_id: "s1".into(),
        agent_id:   "a1".into(),
        agent_chain: vec!["a1".into()],
        description: "desc".into(),
        command: command.into(),
        cwd: std::env::temp_dir(),
        timeout_ms: 30_000,
        silent_completion: false,
    }
}

#[tokio::test]
async fn second_run_of_same_command_has_diff_section() {
    let pool = pool_with_table().await;
    let bash_repo = BashJobRepo::new(pool.inner().clone());
    let bus = Arc::new(DomainEventBus::new(64));
    let queue = Arc::new(ContextUpdateQueue::new());
    let data_dir = tempfile::tempdir().unwrap();

    let supervisor = Arc::new(JobSupervisor::new(
        bash_repo.clone(),
        Arc::clone(&bus),
        Arc::clone(&queue),
        data_dir.path().to_path_buf(),
        Arc::new(klynt_sandbox::MacOsSeatbeltRunner::new()),
    ));

    // First spawn — a quick failure.
    let v1 = supervisor.spawn(spec("false")).await.unwrap();
    wait_for_terminal(&supervisor, &v1.id).await;

    // Second spawn — same command, second failure.
    let v2 = supervisor.spawn(spec("false")).await.unwrap();
    wait_for_terminal(&supervisor, &v2.id).await;

    // Drain the queue and find the second completion notification.
    let updates: Vec<_> = queue.drain();
    let body_v2 = updates.iter()
        .filter_map(|u| u.content.as_ref())
        .find(|s: &&String| s.contains(v2.id.as_str()))
        .expect("expected completion body for v2");

    assert!(
        body_v2.contains("Compared to last run of this command"),
        "expected diff section in body, got:\n{body_v2}"
    );
}

async fn wait_for_terminal(supervisor: &JobSupervisor, id: &tools_core::JobId) {
    for _ in 0..50 {
        if !supervisor.list("s1", &["a1".into()], true).await
            .iter().any(|j| &j.id == id)
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("job did not reach terminal state in time");
}
