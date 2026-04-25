use coding_memory::recall::telemetry::RecallInvocationRepo;
use coding_memory::reforge::{SessionEndPass, SessionSummaryRepo};
use storage::StoragePool;

async fn fresh_pool() -> StoragePool {
    let pool = StoragePool::connect_in_memory().await.expect("pool");
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .expect("cog migs");
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .expect("coding migs");
    pool
}

#[tokio::test]
async fn pass_writes_session_summary_under_budget() {
    let pool = fresh_pool().await;
    let summaries = SessionSummaryRepo::new(pool.clone());
    let co_act = cognitive::CoActivationRepo::new(pool.inner().clone());
    let utilization = RecallInvocationRepo::new(pool.clone());
    let pass = SessionEndPass::new(summaries.clone(), co_act, utilization);

    pass.run("session-A", Some("repo:test")).await.expect("run");

    let row = summaries
        .latest_for_session("session-A")
        .await
        .expect("latest")
        .expect("row");
    assert_eq!(row.session_id, "session-A");
    assert!(row.token_count <= 200);
}

#[tokio::test]
async fn pass_dedups_same_problem_hash_fix_attempts() {
    let pool = fresh_pool().await;
    // seed two episodic_memories with kind='fix_attempt' and same problem_hash in metadata
    let ep_repo = cognitive::EpisodicMemoryRepo::new(pool.inner().clone());
    for i in 0..2 {
        let m = cognitive::types::EpisodicMemory {
            id: format!("ep_{i}"),
            domain: "code".into(),
            content: format!("attempt {i}"),
            summary: None,
            importance: 0.5,
            occurred_at: jiff::Timestamp::now().to_string(),
            recorded_at: jiff::Timestamp::now().to_string(),
            stability: 1.0,
            last_accessed: None,
            access_count: 0,
            project_id: None,
            scope_type: "code".into(),
            scope_id: Some("session-B".into()),
            kind: Some("fix_attempt".into()),
            scope_repo_id: Some("repo:test".into()),
            metadata: Some(r#"{"problem_hash":"abc123"}"#.into()),
        };
        ep_repo.insert(&m).await.expect("ep insert");
    }

    let summaries = SessionSummaryRepo::new(pool.clone());
    let co_act = cognitive::CoActivationRepo::new(pool.inner().clone());
    let utilization = RecallInvocationRepo::new(pool.clone());
    let pass = SessionEndPass::new(summaries, co_act, utilization);

    let report = pass.run("session-B", Some("repo:test")).await.expect("run");
    assert_eq!(report.deduped_attempts, 1, "two attempts → one survivor");
}
