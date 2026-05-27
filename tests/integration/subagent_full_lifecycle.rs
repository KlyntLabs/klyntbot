//! End-to-end: exercise the full subagent lifecycle through the tool layer.

use agent::subagent_runtime::{ResumeParams, SpawnParams, SubagentRuntime};
use providers::testing::SingleResponseProvider;
use storage::{Repos, StoragePool};

#[tokio::test]
async fn full_lifecycle_spawn_list_resume_kill() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repos = Repos::from_pool(&pool);

    // Insert parent session.
    sqlx::query("INSERT INTO sessions (key, mode) VALUES ('parent-A', 'assistant')")
        .execute(pool.inner())
        .await
        .unwrap();

    let rt = SubagentRuntime {
        repo: repos.subagent_instances.clone(),
        sessions: repos.sessions.clone(),
        active: agent::subagent_runtime::ActiveSubagentRegistry::new(),
        provider: SingleResponseProvider::dyn_arc("done"),
        workspace: std::path::PathBuf::from("/tmp"),
        model: "test".to_string(),
        tool_kit: None,
        hook_engine: None,
        job_supervisor: None,
        event_tx: std::sync::Arc::new(std::sync::Mutex::new(None)),
    };

    // 1. Spawn with a tight cap.
    let r1 = rt
        .spawn(SpawnParams {
            description: "analyze agent dir".to_string(),
            prompt: "read all files in /tmp/agent".to_string(),
            model: None,
            max_turns: Some(5),
            workspace_path: std::path::PathBuf::from("/tmp"),
            parent_session_id: "parent-A".to_string(),
            parent_agent_id: None,
        })
        .await
        .unwrap();
    assert_eq!(r1.status, storage::rows::SubagentStatus::Idle);
    assert!(!r1.agent_id.is_empty());

    // 2. List should find the instance (now idle).
    let rows = rt.list(None, None).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].agent_id, r1.agent_id);

    // 3. Resume the same instance.
    let r2 = rt
        .resume(ResumeParams {
            agent_id: r1.agent_id.clone(),
            prompt: "continue analysis".to_string(),
        })
        .await
        .unwrap();
    assert_eq!(r2.status, storage::rows::SubagentStatus::Idle);
    assert_eq!(r2.agent_id, r1.agent_id);

    // 4. turns_used_total should have accumulated across both runs.
    let row = repos
        .subagent_instances
        .get(&r1.agent_id)
        .await
        .unwrap()
        .unwrap();
    assert!(
        row.turns_used_total >= 2,
        "expected at least 2 total turns, got {}",
        row.turns_used_total
    );

    // 5. Kill should flip to killed.
    let r3 = rt.kill(&r1.agent_id).await.unwrap();
    assert_eq!(r3.status, storage::rows::SubagentStatus::Killed);

    // 6. Resume on killed should fail.
    let err = rt
        .resume(ResumeParams {
            agent_id: r1.agent_id.clone(),
            prompt: "try again".to_string(),
        })
        .await;
    assert!(err.is_err());
    let msg = err.unwrap_err().to_string();
    assert!(
        msg.contains("not resumable"),
        "expected 'not resumable' error, got: {}",
        msg
    );
}

#[tokio::test]
async fn spawn_with_parent_agent_id_filters_correctly() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repos = Repos::from_pool(&pool);

    sqlx::query("INSERT INTO sessions (key, mode) VALUES ('p1', 'assistant'), ('p2', 'assistant')")
        .execute(pool.inner())
        .await
        .unwrap();

    let rt = SubagentRuntime {
        repo: repos.subagent_instances.clone(),
        sessions: repos.sessions.clone(),
        active: agent::subagent_runtime::ActiveSubagentRegistry::new(),
        provider: SingleResponseProvider::dyn_arc("done"),
        workspace: std::path::PathBuf::from("/tmp"),
        model: "test".to_string(),
        tool_kit: None,
        hook_engine: None,
        job_supervisor: None,
        event_tx: std::sync::Arc::new(std::sync::Mutex::new(None)),
    };

    let r1 = rt
        .spawn(SpawnParams {
            description: "task 1".to_string(),
            prompt: "do thing 1".to_string(),
            model: None,
            max_turns: Some(5),
            workspace_path: std::path::PathBuf::from("/tmp"),
            parent_session_id: "p1".to_string(),
            parent_agent_id: None,
        })
        .await
        .unwrap();

    // Spawn a child subagent referencing r1 as parent.
    let r2 = rt
        .spawn(SpawnParams {
            description: "task 2".to_string(),
            prompt: "do thing 2".to_string(),
            model: None,
            max_turns: Some(5),
            workspace_path: std::path::PathBuf::from("/tmp"),
            parent_session_id: "p2".to_string(),
            parent_agent_id: Some(r1.agent_id.clone()),
        })
        .await
        .unwrap();

    // List by parent_agent_id should filter correctly.
    let rows = rt.list(Some(&r1.agent_id), None).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].agent_id, r2.agent_id);

    // List all should return both.
    let rows = rt.list(None, None).await.unwrap();
    assert_eq!(rows.len(), 2);
}
