//! Attach + detach writes two episodic_memories rows for the same job.

use std::sync::Arc;

use ai_core::AiSignal;
use bus::DomainEventBus;
use cognitive::mirror::sources::BackgroundJobSignalSource;
use cognitive::repos::EpisodicMemoryRepo;
use feature_coding_bash::JobSupervisor;
use klynt_sandbox::MacOsSeatbeltRunner;
use storage::repos::BashJobRepo;
use tools_core::{JobSpec, JobSupervisorHandle};

#[tokio::test]
#[cfg(target_os = "macos")]
async fn attach_lifecycle_writes_two_episodes() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    sqlx::query(&feature_coding_bash::migrations::coding_background_jobs_migration().sql)
        .execute(pool.inner())
        .await
        .unwrap();
    // Episodic table migration:
    // The cognitive crate provides its own migration; load it here.
    for m in cognitive::repos::cognitive_migrations() {
        sqlx::query(&m.sql).execute(pool.inner()).await.unwrap();
    }
    let bash_repo = Arc::new(BashJobRepo::new(pool.inner().clone()));
    let ep_repo = EpisodicMemoryRepo::new(pool.inner().clone());
    let bus = Arc::new(DomainEventBus::new(256));
    let sup = Arc::new(JobSupervisor::new(
        (*bash_repo).clone(),
        bus.clone(),
        Arc::new(bus::context_updates::ContextUpdateQueue::new()),
        tempfile::tempdir().unwrap().into_path(),
        Arc::new(MacOsSeatbeltRunner::new()),
    ));
    let source = BackgroundJobSignalSource::new(ep_repo.clone(), bash_repo.clone());
    let view = sup
        .spawn(JobSpec {
            session_id: "s1".into(),
            agent_id: "a1".into(),
            agent_chain: vec!["a1".into()],
            description: "probe".into(),
            command: "sleep 5".into(),
            cwd: std::env::temp_dir(),
            timeout_ms: 30_000,
            silent_completion: true,
            tty: true,
            tty_rows: Some(24),
            tty_cols: Some(80),
        })
        .await
        .expect("spawn");
    sup.attach(&view.id).await.expect("attach");
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    sup.detach(&view.id).await.expect("detach");

    // Drive the source manually via published events.
    // (In production, ai_pipeline does this; in this test we shortcut to verify
    // that the source's `accumulate` writes episodes.)
    use bus::BashJobEvent;
    let started = AiSignal {
        domain: ai_core::RecallDomain::Mirror,
        event_kind: "BashJob.AttachStarted",
        importance: 0.4,
        salience: ai_core::SalienceVerdict::Accumulate,
        content: "".into(),
        entity: None,
        timestamp: jiff::Timestamp::now(),
        raw_event: Some(bus::DomainEvent::BashJob(BashJobEvent::AttachStarted {
            job_id: view.id.0.clone(),
            thread_id: "s1".into(),
            agent_id: "a1".into(),
            timestamp: jiff::Timestamp::now(),
        })),
        metrics: ai_core::AiMetrics::default(),
        coaching_signal: false,
        coaching_rule: None,
        metric_samples: vec![],
    };
    let ended = AiSignal {
        domain: ai_core::RecallDomain::Mirror,
        event_kind: "BashJob.AttachEnded",
        importance: 0.4,
        salience: ai_core::SalienceVerdict::Accumulate,
        content: "".into(),
        entity: None,
        timestamp: jiff::Timestamp::now(),
        raw_event: Some(bus::DomainEvent::BashJob(BashJobEvent::AttachEnded {
            job_id: view.id.0.clone(),
            thread_id: "s1".into(),
            agent_id: "a1".into(),
            timestamp: jiff::Timestamp::now(),
            duration_ms: 100,
        })),
        metrics: ai_core::AiMetrics::default(),
        coaching_signal: false,
        coaching_rule: None,
        metric_samples: vec![],
    };
    {
        use ai_core::MirrorSignalSource;
        source.accumulate(&started).await.unwrap();
        source.accumulate(&ended).await.unwrap();
    }
    let rows = ep_repo
        .list_by_kinds(&["bash_job_attach"], None, 100)
        .await
        .expect("list episodes");
    assert!(rows.len() >= 2, "expected at least 2 attach episodes");
    let _ = sup.stop(&view.id, "cleanup").await;
}
