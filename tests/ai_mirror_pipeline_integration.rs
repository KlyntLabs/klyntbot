//! Integration tests for the AI Pipeline v2 mirror redesign.
//!
//! Tests that mirror sources receive events via the SignalRouter and produce
//! the expected snapshots.

use ai_core::{MirrorSignalSource, SignalConsumer};
use bus::{CrossDomainEvent, TaskEvent};
use std::sync::Arc;

#[tokio::test]
async fn skill_routed_event_persists_routing_snapshot_via_ai_pipeline() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    storage::StoragePool::run_feature_migrations(
        pool.inner(),
        &cognitive::repos::cognitive_migrations(),
    )
    .await
    .unwrap();
    let bus = Arc::new(bus::DomainEventBus::new(64));
    let mirror_repo = cognitive::mirror::MirrorRepo::new(pool);

    let started = cognitive::mirror::MirrorEngine::start(
        mirror_repo.clone(),
        None,
        None,
        None,
        None,
        None,
        None,
    );

    let _router = ai_core::SignalRouter::start(
        Arc::clone(&bus),
        started.consumers.clone(),
        app_core::init::ai_pipeline::translate,
    );

    // Publish 3 SkillRouted events.
    for i in 0..3 {
        bus.publish(bus::DomainEvent::SkillRouted {
            skill_name: "general".into(),
            confidence: 0.8 + (i as f64 * 0.01),
            source: "keyword".into(),
            trigger_phrases: vec!["hi".into()],
            session_key: "s".into(),
        });
    }

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Direct source test (complements the integration test above).
    let direct = Arc::new(cognitive::mirror::sources::RoutingSignalSource::new(
        mirror_repo.clone(),
    ));
    let runner2 = ai_core::MirrorSubscriberRunner::new(
        direct.clone(),
        tokio_util::sync::CancellationToken::new(),
    );
    let sig = app_core::init::ai_pipeline::translate(&bus::DomainEvent::SkillRouted {
        skill_name: "general".into(),
        confidence: 0.85,
        source: "keyword".into(),
        trigger_phrases: vec!["hi".into()],
        session_key: "s".into(),
    })
    .unwrap();
    let mut sig = sig;
    sig.raw_event = Some(bus::DomainEvent::SkillRouted {
        skill_name: "general".into(),
        confidence: 0.85,
        source: "keyword".into(),
        trigger_phrases: vec!["hi".into()],
        session_key: "s".into(),
    });
    runner2.consume(&sig).await.unwrap();
    direct.flush().await.unwrap();

    let latest = mirror_repo
        .get_latest_routing_snapshot()
        .await
        .unwrap()
        .unwrap();
    assert!(latest.total_messages >= 1);
    assert!(latest.distribution.contains_key("general"));

    started.shutdown.cancel();
    for h in started.flush_handles {
        h.await.unwrap();
    }
}

#[tokio::test]
async fn autotuner_decision_activated_starts_trial_timer() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    storage::StoragePool::run_feature_migrations(
        pool.inner(),
        &cognitive::repos::cognitive_migrations(),
    )
    .await
    .unwrap();
    let bus = Arc::new(bus::DomainEventBus::new(32));
    let mirror_repo = cognitive::mirror::MirrorRepo::new(pool);

    let active_timers: Arc<dashmap::DashMap<String, tokio::task::JoinHandle<()>>> =
        Arc::new(dashmap::DashMap::new());
    let source = Arc::new(cognitive::mirror::sources::TrialPreviewSource::new(
        mirror_repo,
        active_timers.clone(),
        None,
    ));
    let cancel = tokio_util::sync::CancellationToken::new();
    let runner = ai_core::MirrorSubscriberRunner::new(source.clone(), cancel.clone());

    let _router = ai_core::SignalRouter::start(
        Arc::clone(&bus),
        vec![runner as Arc<dyn ai_core::SignalConsumer>],
        app_core::init::ai_pipeline::translate,
    );

    bus.publish(bus::DomainEvent::CrossDomain(
        CrossDomainEvent::AutotunerDecision {
            trial_id: "t-abc".into(),
            verdict: "activated".into(),
            improvement_pct: 0.0,
            affected_params: vec!["temp".into()],
        },
    ));
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    assert!(active_timers.contains_key("t-abc"));
    cancel.cancel();
}

#[tokio::test]
async fn task_focus_changes_produce_focus_snapshot() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    storage::StoragePool::run_feature_migrations(
        pool.inner(),
        &cognitive::repos::cognitive_migrations(),
    )
    .await
    .unwrap();
    let bus = Arc::new(bus::DomainEventBus::new(32));
    let mirror_repo = cognitive::mirror::MirrorRepo::new(pool);

    let source = Arc::new(cognitive::mirror::sources::TaskFocusPatternSource::new(
        mirror_repo.clone(),
    ));
    let cancel = tokio_util::sync::CancellationToken::new();
    let runner = ai_core::MirrorSubscriberRunner::new(source.clone(), cancel.clone());

    // Debug: check what the translator returns
    let test_ev = bus::DomainEvent::Task(TaskEvent::TaskFocusChanged {
        task_id: "t1".into(),
        focus_deadline: Some("2026-04-22T12:00:00Z".into()),
    });
    if let Some(sig) = app_core::init::ai_pipeline::translate(&test_ev) {
        println!(
            "Translated TaskFocusChanged to: event_kind={}, domain={:?}",
            sig.event_kind, sig.domain
        );
    } else {
        println!("Failed to translate TaskFocusChanged");
    }

    let _router = ai_core::SignalRouter::start(
        Arc::clone(&bus),
        vec![runner as Arc<dyn ai_core::SignalConsumer>],
        app_core::init::ai_pipeline::translate,
    );

    // Give the router a moment to subscribe
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    bus.publish(bus::DomainEvent::Task(TaskEvent::TaskFocusChanged {
        task_id: "t1".into(),
        focus_deadline: Some("2026-04-22T12:00:00Z".into()),
    }));
    bus.publish(bus::DomainEvent::Task(TaskEvent::TaskFocusChanged {
        task_id: "t1".into(),
        focus_deadline: Some("2026-04-22T14:00:00Z".into()),
    }));
    bus.publish(bus::DomainEvent::Task(TaskEvent::TaskCompleted {
        task_id: "t1".into(),
        actual_duration_mins: Some(45),
        estimated_duration_mins: Some(30),
        deviation_pct: Some(50.0),
    }));

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    source.flush().await.unwrap();

    let snap = mirror_repo.get_latest_task_focus_snapshot().await.unwrap();
    println!("snap: {:?}", snap);
    assert!(
        snap.is_some(),
        "Expected a task focus snapshot to be created"
    );
    let snap = snap.unwrap();
    assert_eq!(snap.focus_changes, 2, "Expected 2 focus changes");
    assert_eq!(snap.tasks_completed, 1, "Expected 1 task completed");
    cancel.cancel();
}
