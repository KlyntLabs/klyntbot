//! Phase 2.3b: ExecutionIntelligenceInjector renders affordances for verification
//! verb todos when plan mode is active.

use std::sync::Arc;

use bus::injection::{DynamicInjector, InjectorContext};
use feature_coding_bash::ExecutionIntelligenceInjector;
use storage::{repos::TodoRepo, StoragePool};
use tools_core::{JobError, JobId, JobSpec, JobStatus, JobSupervisorHandle, JobView, RingRead};

#[derive(Debug)]
struct StubSupervisor;

#[async_trait::async_trait]
impl JobSupervisorHandle for StubSupervisor {
    async fn spawn(&self, _: JobSpec) -> Result<JobView, JobError> {
        unimplemented!()
    }
    async fn output_delta(&self, _: &JobId, _: u64, _: bool, _: u64) -> Result<RingRead, JobError> {
        unimplemented!()
    }
    async fn stop(&self, _: &JobId, _: &str) -> Result<JobView, JobError> {
        unimplemented!()
    }
    async fn list(&self, _: &str, _: &[String], _: bool) -> Vec<JobView> {
        vec![]
    }
}

struct PlanCtx {
    thread_id: String,
    agent_id: String,
    chain: Vec<String>,
    plan_active: bool,
}

impl InjectorContext for PlanCtx {
    fn thread_id(&self) -> &str {
        &self.thread_id
    }
    fn agent_id(&self) -> &str {
        &self.agent_id
    }
    fn plan_mode_active(&self) -> bool {
        self.plan_active
    }
    fn plan_session_id(&self) -> Option<&str> {
        None
    }
    fn agent_chain(&self) -> &[String] {
        &self.chain
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn renders_affordance_for_verification_verb_todos() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    // Apply todo migration manually:
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS coding_todos (
            thread_id TEXT NOT NULL,
            agent_id TEXT NOT NULL,
            items_json TEXT NOT NULL DEFAULT '[]',
            proposed_in_plan_session TEXT,
            updated_at TEXT NOT NULL,
            PRIMARY KEY (thread_id, agent_id)
        );
    "#,
    )
    .execute(pool.inner())
    .await
    .unwrap();

    let todo_repo = TodoRepo::new(pool.inner().clone());
    // Insert two todos: one Run, one Refactor.
    todo_repo.upsert("t1", "a1",
        &serde_json::to_string(&serde_json::json!([
            {"id":"x1","title":"Run integration tests","status":"pending","concurrency":"safe"},
            {"id":"x2","title":"Refactor the supervisor","status":"pending","concurrency":"safe"},
        ])).unwrap(),
        None,
    ).await.unwrap();

    let supervisor: Arc<dyn JobSupervisorHandle> = Arc::new(StubSupervisor);
    let injector = ExecutionIntelligenceInjector::new(todo_repo, supervisor);

    let ctx = PlanCtx {
        thread_id: "t1".into(),
        agent_id: "a1".into(),
        chain: vec!["a1".into()],
        plan_active: true,
    };

    let updates = injector.collect(&ctx);
    assert_eq!(updates.len(), 1, "expected exactly 1 update");
    let body = updates[0].content.as_ref().unwrap();
    assert!(body.contains("Run integration tests"));
    assert!(!body.contains("Refactor the supervisor"));
}

#[tokio::test(flavor = "multi_thread")]
async fn no_affordance_when_plan_mode_inactive() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let todo_repo = TodoRepo::new(pool.inner().clone());
    let supervisor: Arc<dyn JobSupervisorHandle> = Arc::new(StubSupervisor);
    let injector = ExecutionIntelligenceInjector::new(todo_repo, supervisor);

    let ctx = PlanCtx {
        thread_id: "t1".into(),
        agent_id: "a1".into(),
        chain: vec!["a1".into()],
        plan_active: false,
    };

    assert!(injector.collect(&ctx).is_empty());
}
