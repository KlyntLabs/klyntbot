//! Crash recovery: a stale 'running' row gets swept to 'failed' at startup.

use storage::{Repos, StoragePool};

#[tokio::test]
async fn stale_running_row_flips_to_failed() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repos = Repos::from_pool(&pool);
    sqlx::query("INSERT INTO sessions (key, mode) VALUES ('p', 'assistant'), ('s', 'subagent')")
        .execute(pool.inner()).await.unwrap();
    repos.subagent_instances.insert(&storage::repos::NewSubagentInstance {
        agent_id: "ag-stale".to_string(),
        session_id: "s".to_string(),
        parent_agent_id: None,
        description: "x".to_string(),
        model: None,
        workspace_path: "/tmp".to_string(),
        turn_cap: 500,
    }).await.unwrap();
    sqlx::query("UPDATE subagent_instances SET updated_at = (unixepoch('now') * 1000) - 600000 WHERE agent_id = 'ag-stale'")
        .execute(pool.inner()).await.unwrap();

    let n = repos.subagent_instances.sweep_zombies(300_000).await.unwrap();
    assert_eq!(n, 1);
    let row = repos.subagent_instances.get("ag-stale").await.unwrap().unwrap();
    assert_eq!(row.status, "failed");
    assert!(row.partial_summary.is_some());
}
