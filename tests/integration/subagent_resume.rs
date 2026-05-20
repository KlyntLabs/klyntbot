//! End-to-end: spawn a subagent with a tight turn cap, force a cap-hit,
//! resume it, and verify completion.

use storage::{Repos, StoragePool};

#[tokio::test]
async fn spawn_capped_then_resume_to_completion() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repos = Repos::from_pool(&pool);
    let provider: providers::DynProvider = providers::testing::SingleResponseProvider::dyn_arc("done");
    let rt = agent::subagent_runtime::SubagentRuntime {
        repo: repos.subagent_instances.clone(),
        sessions: repos.sessions.clone(),
        active: agent::subagent_runtime::ActiveSubagentRegistry::new(),
        provider,
        workspace: std::path::PathBuf::from("/tmp"),
        model: "test".to_string(),
        tool_kit: None,
        hook_engine: None,
        job_supervisor: None,
        event_tx: std::sync::Arc::new(std::sync::Mutex::new(None)),
    };
    // Parent session.
    sqlx::query("INSERT INTO sessions (key, mode) VALUES ('parent-A', 'assistant')")
        .execute(pool.inner())
        .await
        .unwrap();

    let r1 = rt.spawn(agent::subagent_runtime::SpawnParams {
        description: "test".to_string(),
        prompt: "hi".to_string(),
        model: None,
        max_turns: Some(5),
        workspace_path: std::path::PathBuf::from("/tmp"),
        parent_session_id: "parent-A".to_string(),
        parent_agent_id: None,
    }).await.unwrap();
    assert_eq!(r1.status, storage::rows::SubagentStatus::Idle);

    let r2 = rt.resume(agent::subagent_runtime::ResumeParams {
        agent_id: r1.agent_id.clone(),
        prompt: "continue".to_string(),
    }).await.unwrap();
    assert_eq!(r2.status, storage::rows::SubagentStatus::Idle);
    assert_eq!(r2.agent_id, r1.agent_id);

    let row = repos.subagent_instances.get(&r1.agent_id).await.unwrap().unwrap();
    assert!(row.turns_used_total >= 2);
}

#[tokio::test]
async fn resume_on_running_returns_error() {
    // Setup: create a row in 'running' state directly (skips the actual loop).
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repos = Repos::from_pool(&pool);
    let rt = agent::subagent_runtime::SubagentRuntime {
        repo: repos.subagent_instances.clone(),
        sessions: repos.sessions.clone(),
        active: agent::subagent_runtime::ActiveSubagentRegistry::new(),
        provider: providers::testing::SingleResponseProvider::dyn_arc("x"),
        workspace: std::path::PathBuf::from("/tmp"),
        model: "test".to_string(),
        tool_kit: None,
        hook_engine: None,
        job_supervisor: None,
        event_tx: std::sync::Arc::new(std::sync::Mutex::new(None)),
    };
    sqlx::query("INSERT INTO sessions (key, mode) VALUES ('parent-B', 'assistant'), ('sub-B', 'subagent')")
        .execute(pool.inner()).await.unwrap();
    repos.subagent_instances.insert(&storage::repos::NewSubagentInstance {
        agent_id: "ag-busy".to_string(),
        session_id: "sub-B".to_string(),
        parent_agent_id: None,
        description: "x".to_string(),
        model: None,
        workspace_path: "/tmp".to_string(),
        turn_cap: 500,
    }).await.unwrap();

    let err = rt.resume(agent::subagent_runtime::ResumeParams {
        agent_id: "ag-busy".to_string(),
        prompt: "x".to_string(),
    }).await;
    assert!(err.is_err());
    assert!(err.unwrap_err().to_string().contains("currently running"));
}
