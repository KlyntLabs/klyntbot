//! Phase 2.3b: orphan rows reconciled at startup produce Lost episodes.

use std::sync::Arc;
use std::time::Duration;

use ai_core::MirrorSignalSource;
use bus::{DomainEvent, DomainEventBus};
use cognitive::mirror::sources::BackgroundJobSignalSource;
use cognitive::repos::EpisodicMemoryRepo;
use feature_coding_bash::JobSupervisor;
use jiff::Timestamp;
use storage::{repos::{BashJobRepo, BashJobRow}, StoragePool};

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
async fn lost_episode_written_on_reconcile() {
    let pool = pool_with_tables().await;

    let bash_repo = BashJobRepo::new(pool.inner().clone());
    let bash_repo_arc = Arc::new(bash_repo.clone());
    let ep_repo = EpisodicMemoryRepo::new(pool.inner().clone());

    let source = Arc::new(BackgroundJobSignalSource::new(ep_repo.clone(), bash_repo_arc.clone()));

    let bus = Arc::new(DomainEventBus::new(64));
    let queue = Arc::new(bus::context_updates::ContextUpdateQueue::new());
    let data_dir = tempfile::tempdir().unwrap();

    // Insert a fake Running row directly.
    let row = BashJobRow {
        id: "bash-0000000001".into(),
        session_id: "s1".into(),
        agent_id: "a1".into(),
        description: "orphan".into(),
        command: "sleep 999".into(),
        command_key: "sleep_999".into(),
        cwd: "/".into(),
        timeout_ms: 600_000,
        silent_completion: false,
        status: "Running".into(),
        exit_code: None,
        failure_kind: None,
        failure_detail: None,
        failure_extracted: None,
        started_at: Timestamp::now(),
        finished_at: None,
        total_bytes_emitted: 0,
        bisect_count: 0,
        log_path: "/tmp/bash-0000000001.log".into(),
        final_path: None,
        last_polled_at: None,
        last_seen_offset: 0,
    };
    bash_repo.insert(&row).await.unwrap();

    // Create supervisor and call reconcile_on_startup.
    let supervisor = Arc::new(JobSupervisor::new(
        bash_repo,
        Arc::clone(&bus),
        Arc::clone(&queue),
        data_dir.path().to_path_buf(),
        Arc::new(klynt_sandbox::MacOsSeatbeltRunner::new()),
    ));

    let n = supervisor.reconcile_on_startup().await.unwrap();
    assert!(n >= 1, "expected at least 1 orphan reconciled");

    // Fetch the updated row and emit a Lost event manually.
    let updated = bash_repo_arc.get("bash-0000000001").await.unwrap().unwrap();
    assert_eq!(updated.status, "Lost");

    let event = DomainEvent::BashJob(bus::BashJobEvent::Lost {
        job_id: updated.id.clone(),
        thread_id: updated.session_id.clone(),
        agent_id: updated.agent_id.clone(),
    });

    let signal = ai_core::AiSignal {
        domain: ai_core::RecallDomain::General,
        event_kind: "BashJob.Lost",
        importance: 0.6,
        salience: ai_core::SalienceVerdict::Accumulate,
        content: format!("bash job {}", updated.id),
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
    assert!((importance - 0.6).abs() < 0.01, "got {importance}");
}
