//! Phase 2.3b: ExecutionIntelligenceInjector suppresses affordances for todos
//! whose title is already covered by an active background job.

use std::sync::Arc;

use bus::injection::{DynamicInjector, InjectorContext};
use feature_coding_bash::ExecutionIntelligenceInjector;
use jiff::Timestamp;
use storage::{repos::TodoRepo, StoragePool};
use tools_core::{
    JobError, JobId, JobSpec, JobStatus, JobSupervisorHandle, JobView, RingRead,
};

#[derive(Debug)]
struct OneActiveJobSupervisor;

#[async_trait::async_trait]
impl JobSupervisorHandle for OneActiveJobSupervisor {
    async fn spawn(&self, _: JobSpec) -> Result<JobView, JobError> { unimplemented!() }
    async fn output_delta(&self, _: &JobId, _: u64, _: bool, _: u64
    ) -> Result<RingRead, JobError> { unimplemented!() }
    async fn stop(&self, _: &JobId, _: &str) -> Result<JobView, JobError> { unimplemented!() }
    async fn list(&self, _: &str, _: &[String], _: bool) -> Vec<JobView> {
        vec![JobView {
            id: JobId("bash-x".into()),
            session_id: "t1".into(),
            agent_id: "a1".into(),
            description: "Run integration tests on the agent crate".into(),
            command: "cargo test -p agent".into(),
            cwd: std::path::PathBuf::from("/"),
            status: JobStatus::Running,
            started_at: Timestamp::now(),
            finished_at: None,
            exit_code: None,
            gate_result: None,
            failure_extracted: None,
            total_bytes_emitted: 0,
            bisect_generation: 0,
            last_polled_at: None,
            last_seen_offset: 0,
        }]
    }
}

struct PlanCtx {
    chain: Vec<String>,
}

impl InjectorContext for PlanCtx {
    fn thread_id(&self) -> &str { "t1" }
    fn agent_id(&self) -> &str { "a1" }
    fn plan_mode_active(&self) -> bool { true }
    fn plan_session_id(&self) -> Option<&str> { None }
    fn agent_chain(&self) -> &[String] { &self.chain }
}

#[tokio::test(flavor = "multi_thread")]
async fn affordance_suppressed_when_active_job_covers_todo() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS coding_todos (
            thread_id TEXT NOT NULL,
            agent_id TEXT NOT NULL,
            items_json TEXT NOT NULL DEFAULT '[]',
            proposed_in_plan_session TEXT,
            updated_at TEXT NOT NULL,
            PRIMARY KEY (thread_id, agent_id)
        );
    "#).execute(pool.inner()).await.unwrap();

    let todo_repo = TodoRepo::new(pool.inner().clone());
    todo_repo.upsert("t1", "a1",
        &serde_json::to_string(&serde_json::json!([
            {"id":"x1","title":"Run integration tests","status":"pending","concurrency":"safe"}
        ])).unwrap(),
        None,
    ).await.unwrap();

    let supervisor: Arc<dyn JobSupervisorHandle> = Arc::new(OneActiveJobSupervisor);
    let injector = ExecutionIntelligenceInjector::new(todo_repo, supervisor);
    let ctx = PlanCtx { chain: vec!["a1".into()] };
    assert!(injector.collect(&ctx).is_empty(), "active job should suppress affordance");
}
