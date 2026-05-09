//! Phase 2.3b: a Passed bash job results in an EpisodicMemory row with importance≈0.3.

use std::sync::Arc;
use std::time::Duration;

use ai_core::MirrorSignalSource;
use bus::{DomainEvent, DomainEventBus};
use cognitive::mirror::sources::BackgroundJobSignalSource;
use cognitive::repos::EpisodicMemoryRepo;
use feature_coding_bash::JobSupervisor;
use storage::{repos::BashJobRepo, StoragePool};
use tools_core::{JobSpec, JobSupervisorHandle};

async fn pool_with_tables() -> StoragePool {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let m1 = feature_coding_bash::migrations::coding_background_jobs_migration();
    sqlx::query(&m1.sql).execute(pool.inner()).await.unwrap();
    sqlx::query(include_str!("../../cognitive/migrations/001_cognitive_tables.sql"))
        .execute(pool.inner()).await.unwrap();
    sqlx::query(include_str!("../../cognitive/migrations/014_hierarchical_episodics.sql"))
        .execute(pool.inner()).await.unwrap();
    sqlx::query(include_str!("../../cognitive/migrations/016_episodic_actor_id.sql"))
        .execute(pool.inner()).await.unwrap();
    pool
}

#[tokio::test(flavor = "multi_thread")]
async fn passed_job_writes_episode_with_importance_0_3() {
    let pool = pool_with_tables().await;

    let bash_repo = BashJobRepo::new(pool.inner().clone());
    let bash_repo_arc = Arc::new(bash_repo.clone());
    let ep_repo = EpisodicMemoryRepo::new(pool.inner().clone());
    let ep_repo_clone = ep_repo.clone();

    let source = Arc::new(BackgroundJobSignalSource::new(ep_repo, bash_repo_arc.clone()));

    let bus = Arc::new(DomainEventBus::new(64));
    let queue = Arc::new(bus::context_updates::ContextUpdateQueue::new());
    let data_dir = tempfile::tempdir().unwrap();

    let supervisor = Arc::new(JobSupervisor::new(
        bash_repo,
        Arc::clone(&bus),
        Arc::clone(&queue),
        data_dir.path().to_path_buf(),
        Arc::new(klynt_sandbox::MacOsSeatbeltRunner::new()),
    ));

    let v: tools_core::JobView = supervisor.spawn(JobSpec {
        session_id: "s1".into(),
        agent_id:   "a1".into(),
        agent_chain: vec!["a1".into()],
        description: "test pass".into(),
        command: "true".into(),
        cwd: std::env::temp_dir(),
        timeout_ms: 30_000,
        silent_completion: false,
    }).await.unwrap();

    tokio::time::sleep(Duration::from_secs(2)).await;

    let row = bash_repo_arc.get(v.id.as_str()).await.unwrap().unwrap();
    let event = DomainEvent::BashJob(bus::BashJobEvent::Completed {
        job_id: row.id.clone(),
        thread_id: row.session_id.clone(),
        agent_id: row.agent_id.clone(),
        exit_code: 0,
        duration_ms: 100,
    });

    let signal = ai_core::AiSignal {
        domain: ai_core::RecallDomain::General,
        event_kind: "BashJob.Completed",
        importance: 0.3,
        salience: ai_core::SalienceVerdict::Accumulate,
        content: format!("bash job {}", row.id),
        entity: None,
        timestamp: jiff::Timestamp::now(),
        raw_event: Some(event),
        metrics: ai_core::AiMetrics::default(),
        coaching_signal: false,
        coaching_rule: None,
        metric_samples: vec![],
    };

    source.accumulate(&signal).await.unwrap();

    let importance: f64 = sqlx::query_scalar(
        "SELECT importance FROM episodic_memories WHERE kind = 'bash_job' LIMIT 1"
    )
    .fetch_one(pool.inner()).await.unwrap();
    assert!((importance - 0.3).abs() < 0.01, "got {importance}");
}
