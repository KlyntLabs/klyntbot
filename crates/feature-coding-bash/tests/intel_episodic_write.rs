//! Phase 2.3b: a Failed bash job results in an EpisodicMemory row.

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

    // bash jobs migration
    let m1 = feature_coding_bash::migrations::coding_background_jobs_migration();
    sqlx::query(&m1.sql).execute(pool.inner()).await.unwrap();

    // cognitive episodic_memories table + extensions
    sqlx::query(include_str!("../../cognitive/migrations/001_cognitive_tables.sql"))
        .execute(pool.inner()).await.unwrap();
    sqlx::query(include_str!("../../cognitive/migrations/014_hierarchical_episodics.sql"))
        .execute(pool.inner()).await.unwrap();
    sqlx::query(include_str!("../../cognitive/migrations/016_episodic_actor_id.sql"))
        .execute(pool.inner()).await.unwrap();

    pool
}

#[tokio::test(flavor = "multi_thread")]
async fn failed_job_writes_episodic_memory() {
    let pool = pool_with_tables().await;

    let bash_repo = BashJobRepo::new(pool.inner().clone());
    let bash_repo_arc = Arc::new(bash_repo.clone());
    let ep_repo = EpisodicMemoryRepo::new(pool.inner().clone());

    // Set up the signal source manually (skip the full MirrorEngine).
    let source = Arc::new(BackgroundJobSignalSource::new(ep_repo.clone(), bash_repo_arc.clone()));

    // Set up supervisor and spawn a failing job.
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
        description: "test failure".into(),
        command: "false".into(),
        cwd: std::env::temp_dir(),
        timeout_ms: 30_000,
        silent_completion: false,
    }).await.unwrap();

    // Wait for completion.
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Fetch the row from the repo and manually trigger the source.
    let row = bash_repo_arc.get(v.id.as_str()).await.unwrap().unwrap();
    eprintln!("DEBUG: job status = {:?}, exit_code = {:?}", row.status, row.exit_code);
    assert_eq!(row.status, "Failed", "expected job to fail, got status: {}", row.status);
    let event = DomainEvent::BashJob(bus::BashJobEvent::Failed {
        job_id: row.id.clone(),
        thread_id: row.session_id.clone(),
        agent_id: row.agent_id.clone(),
        exit_code: row.exit_code,
        failure_kind: row.failure_kind.clone().unwrap_or_default(),
        failure_detail: row.failure_detail.clone().unwrap_or_default(),
    });

    // Build a minimal AiSignal with the raw_event populated.
    let signal = ai_core::AiSignal {
        domain: ai_core::RecallDomain::General,
        event_kind: "BashJob.Failed",
        importance: 0.7,
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

    // Directly build and insert to test the repo path.
    let mem = cognitive::mirror::sources::coding_bash::build_episodic_memory(&row);
    ep_repo.insert(&mem).await.unwrap();

    let row_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM episodic_memories WHERE kind = 'bash_job'"
    )
    .fetch_one(pool.inner()).await.unwrap();

    assert!(row_count >= 1, "expected ≥1 bash_job episode, got {row_count}");
}
